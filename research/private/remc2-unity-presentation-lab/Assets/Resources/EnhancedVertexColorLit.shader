Shader "MagicCarpet/Enhanced/VertexColorLit"
{
    Properties
    {
        [MainColor] _BaseColor ("Base Color", Color) = (1, 1, 1, 1)
        [HDR] _EmissionColor ("Emission Color", Color) = (0, 0, 0, 0)
        _EmissionStrength ("Emission Strength", Range(0, 2)) = 0
        _AmbientBoost ("Night Ambient Boost", Range(0, 1)) = 0.2
        _SpecularStrength ("Specular Strength", Range(0, 1)) = 0.12
    }

    SubShader
    {
        Tags
        {
            "RenderType" = "Opaque"
            "Queue" = "Geometry"
            "RenderPipeline" = "UniversalPipeline"
        }

        Pass
        {
            Name "ForwardLit"
            Tags { "LightMode" = "UniversalForward" }

            HLSLPROGRAM
            #pragma vertex Vert
            #pragma fragment Frag
            #pragma multi_compile _ _MAIN_LIGHT_SHADOWS _MAIN_LIGHT_SHADOWS_CASCADE _MAIN_LIGHT_SHADOWS_SCREEN
            #pragma multi_compile _ _ADDITIONAL_LIGHTS_VERTEX _ADDITIONAL_LIGHTS
            #pragma multi_compile_fragment _ _ADDITIONAL_LIGHT_SHADOWS
            #pragma multi_compile_fragment _ _SHADOWS_SOFT

            #include "Packages/com.unity.render-pipelines.universal/ShaderLibrary/Core.hlsl"
            #include "Packages/com.unity.render-pipelines.universal/ShaderLibrary/Lighting.hlsl"

            CBUFFER_START(UnityPerMaterial)
                half4 _BaseColor;
                half4 _EmissionColor;
                half _EmissionStrength;
                half _AmbientBoost;
                half _SpecularStrength;
            CBUFFER_END

            struct Attributes
            {
                float4 positionOS : POSITION;
                float3 normalOS : NORMAL;
                float4 color : COLOR;
            };

            struct Varyings
            {
                float4 positionHCS : SV_POSITION;
                float3 positionWS : TEXCOORD0;
                half3 normalWS : TEXCOORD1;
                half4 color : TEXCOORD2;
            };

            Varyings Vert(Attributes input)
            {
                Varyings output;
                VertexPositionInputs positionInputs = GetVertexPositionInputs(input.positionOS.xyz);
                VertexNormalInputs normalInputs = GetVertexNormalInputs(input.normalOS);
                output.positionHCS = positionInputs.positionCS;
                output.positionWS = positionInputs.positionWS;
                output.normalWS = normalInputs.normalWS;
                output.color = input.color;
                return output;
            }

            half3 EvaluateLight(Light light, half3 normalWS, half3 viewDirectionWS, half3 baseColor)
            {
                half diffuse = saturate(dot(normalWS, light.direction));
                half3 halfDirection = SafeNormalize(light.direction + viewDirectionWS);
                half specular = pow(saturate(dot(normalWS, halfDirection)), 32h) * _SpecularStrength;
                half attenuation = light.distanceAttenuation * light.shadowAttenuation;
                return (baseColor * diffuse + specular.xxx) * light.color * attenuation;
            }

            half4 Frag(Varyings input) : SV_Target
            {
                half3 normalWS = NormalizeNormalPerPixel(input.normalWS);
                half3 viewDirectionWS = GetWorldSpaceNormalizeViewDir(input.positionWS);
                half3 baseColor = saturate(input.color.rgb * _BaseColor.rgb);
                half3 lighting = max(SampleSH(normalWS), _AmbientBoost.xxx);

                Light mainLight = GetMainLight(TransformWorldToShadowCoord(input.positionWS));
                lighting += EvaluateLight(mainLight, normalWS, viewDirectionWS, baseColor);

                #if defined(_ADDITIONAL_LIGHTS)
                    uint additionalLightCount = GetAdditionalLightsCount();
                    for (uint index = 0u; index < additionalLightCount; index++)
                    {
                        Light additionalLight = GetAdditionalLight(index, input.positionWS);
                        lighting += EvaluateLight(additionalLight, normalWS, viewDirectionWS, baseColor);
                    }
                #endif

                half3 color = baseColor * lighting;
                color += _EmissionColor.rgb * _EmissionStrength;
                return half4(color, _BaseColor.a * input.color.a);
            }
            ENDHLSL
        }

        Pass
        {
            Name "ShadowCaster"
            Tags { "LightMode" = "ShadowCaster" }
            ZWrite On
            ZTest LEqual

            HLSLPROGRAM
            #pragma vertex ShadowPassVertex
            #pragma fragment ShadowPassFragment
            #include "Packages/com.unity.render-pipelines.universal/ShaderLibrary/Core.hlsl"
            #include "Packages/com.unity.render-pipelines.universal/Shaders/ShadowCasterPass.hlsl"
            ENDHLSL
        }
    }
}
