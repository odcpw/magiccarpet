Shader "MagicCarpet/Enhanced/Terrain"
{
    Properties
    {
        [MainColor] _BaseColor ("Base Color", Color) = (1, 1, 1, 1)
        _DetailScale ("Procedural Detail Scale", Float) = 0.075
        _DetailStrength ("Procedural Detail Strength", Range(0, 1)) = 0.42
        _SlopeTint ("Moonlit Slope Tint", Color) = (0.72, 0.84, 1.08, 1)
        _SlopeStrength ("Slope Tint Strength", Range(0, 1)) = 0.42
        _AmbientBoost ("Night Ambient Boost", Range(0, 2)) = 0.65
        _SpecularStrength ("Moon Specular Strength", Range(0, 1)) = 0.10
        _NormalStrength ("Procedural Relief Strength", Range(0, 1)) = 0.16
        _RidgeStrength ("Procedural Ridge Strength", Range(0, 1)) = 0.24
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
            #pragma multi_compile_fog

            #include "Packages/com.unity.render-pipelines.universal/ShaderLibrary/Core.hlsl"
            #include "Packages/com.unity.render-pipelines.universal/ShaderLibrary/Lighting.hlsl"

            CBUFFER_START(UnityPerMaterial)
                half4 _BaseColor;
                half _DetailScale;
                half _DetailStrength;
                half4 _SlopeTint;
                half _SlopeStrength;
                half _AmbientBoost;
                half _SpecularStrength;
                half _NormalStrength;
                half _RidgeStrength;
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

            half3 EvaluateLight(Light light, half3 normalWS, half3 viewDirectionWS)
            {
                half diffuse = saturate(dot(normalWS, light.direction));
                half3 halfDirection = SafeNormalize(light.direction + viewDirectionWS);
                half specular = pow(saturate(dot(normalWS, halfDirection)), 32.0h) * _SpecularStrength;
                half attenuation = light.distanceAttenuation * light.shadowAttenuation;
                return (diffuse.xxx + specular.xxx) * light.color * attenuation;
            }

            float Hash12(float2 position)
            {
                position = frac(position * float2(123.34, 456.21));
                position += dot(position, position + 45.32);
                return frac(position.x * position.y);
            }

            float ValueNoise(float2 position)
            {
                float2 cell = floor(position);
                float2 fraction = frac(position);
                fraction = fraction * fraction * (3.0 - 2.0 * fraction);
                float lower = lerp(Hash12(cell), Hash12(cell + float2(1.0, 0.0)), fraction.x);
                float upper = lerp(Hash12(cell + float2(0.0, 1.0)), Hash12(cell + 1.0), fraction.x);
                return lerp(lower, upper, fraction.y);
            }

            half4 Frag(Varyings input) : SV_Target
            {
                half3 normalWS = NormalizeNormalPerPixel(input.normalWS);
                half3 viewDirectionWS = GetWorldSpaceNormalizeViewDir(input.positionWS);
                half3 baseColor = saturate(input.color.rgb * _BaseColor.rgb);

                float macro = ValueNoise(input.positionWS.xz * _DetailScale);
                float fine = ValueNoise(input.positionWS.xz * (_DetailScale * 3.5));
                float detail = lerp(macro, fine, 0.32);

                // The terrain mesh is intentionally asset-free.  Reuse the
                // stable scalar field above as a bounded normal perturbation
                // so moonlight breaks across broad relief instead of only
                // changing the albedo.  The finite-difference step is large
                // enough to avoid noisy pixel-scale shimmer at game scale.
                float2 reliefCoord = input.positionWS.xz * max(_DetailScale * 2.4, 0.001);
                const float reliefStep = 0.12;
                float reliefLeft = ValueNoise(reliefCoord - float2(reliefStep, 0.0));
                float reliefRight = ValueNoise(reliefCoord + float2(reliefStep, 0.0));
                float reliefDown = ValueNoise(reliefCoord - float2(0.0, reliefStep));
                float reliefUp = ValueNoise(reliefCoord + float2(0.0, reliefStep));
                float2 reliefGradient = float2(reliefRight - reliefLeft, reliefUp - reliefDown);
                normalWS = SafeNormalize(normalWS + half3(
                    -reliefGradient.x * _NormalStrength,
                    0.0h,
                    -reliefGradient.y * _NormalStrength));

                half variation = lerp(0.68h, 1.34h, detail);
                baseColor *= lerp(1.0h, variation, _DetailStrength);

                half slope = saturate(1.0h - normalWS.y);
                half exposedFace = smoothstep(0.18h, 0.90h, slope);
                baseColor = lerp(
                    baseColor,
                    baseColor * _SlopeTint.rgb,
                    exposedFace * _SlopeStrength);

                // A soft ridge mask adds readable strata to the low-poly
                // surface while remaining a single material pass.  It is
                // deliberately restrained so the night palette remains cool
                // and the original terrain selectors stay legible.
                half ridgeMask = smoothstep(0.58h, 0.94h, 1.0h - abs(2.0h * detail - 1.0h));
                ridgeMask *= saturate(0.18h + slope * 1.15h);
                half3 ridgeTint = lerp(
                    _SlopeTint.rgb * 0.62h,
                    _SlopeTint.rgb * 0.88h,
                    saturate(detail * 1.4h));
                baseColor = lerp(
                    baseColor,
                    baseColor * ridgeTint + ridgeTint * 0.015h,
                    ridgeMask * _RidgeStrength);

                half3 lighting = max(SampleSH(normalWS), _AmbientBoost * half3(0.68h, 0.80h, 1.0h));
                Light mainLight = GetMainLight(TransformWorldToShadowCoord(input.positionWS));
                lighting += EvaluateLight(mainLight, normalWS, viewDirectionWS);

                #if defined(_ADDITIONAL_LIGHTS)
                    uint additionalLightCount = GetAdditionalLightsCount();
                    for (uint index = 0u; index < additionalLightCount; index++)
                    {
                        Light additionalLight = GetAdditionalLight(index, input.positionWS);
                        lighting += EvaluateLight(additionalLight, normalWS, viewDirectionWS);
                    }
                #endif

                half3 color = baseColor * lighting;
                half rim = pow(saturate(1.0h - dot(normalWS, viewDirectionWS)), 2.0h);
                color += _SlopeTint.rgb * (rim * exposedFace * 0.08h);
                color = MixFog(color, ComputeFogFactor(input.positionHCS.z));
                return half4(color, input.color.a * _BaseColor.a);
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
