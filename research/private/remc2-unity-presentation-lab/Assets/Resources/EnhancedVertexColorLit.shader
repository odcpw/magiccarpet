Shader "MagicCarpet/Enhanced/VertexColorLit"
{
    Properties
    {
        [MainColor] _BaseColor ("Base Color", Color) = (1, 1, 1, 1)
        [HDR] _EmissionColor ("Emission Color", Color) = (0, 0, 0, 0)
        _EmissionStrength ("Emission Strength", Range(0, 2)) = 0
        _AmbientBoost ("Night Ambient Boost", Range(0, 1)) = 0.2
        _SpecularStrength ("Specular Strength", Range(0, 1)) = 0.12
        _RimColor ("Night Rim Color", Color) = (0.08, 0.16, 0.52, 1)
        _RimStrength ("Night Rim Strength", Range(0, 1)) = 0
        _DetailScale ("Procedural Detail Scale", Float) = 0.08
        _DetailStrength ("Procedural Detail Strength", Range(0, 1)) = 0
        [HDR] _AccentColor ("Structure Accent Color", Color) = (1, 0.24, 0.04, 1)
        _AccentStrength ("Structure Accent Strength", Range(0, 2)) = 0
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
                half4 _EmissionColor;
                half _EmissionStrength;
                half _AmbientBoost;
                half _SpecularStrength;
                half4 _RimColor;
                half _RimStrength;
                half _DetailScale;
                half _DetailStrength;
                half4 _AccentColor;
                half _AccentStrength;
            CBUFFER_END

            #define ENHANCED_WARM_LIGHT_CAP 4
            float4 _EnhancedWarmLightPositions[ENHANCED_WARM_LIGHT_CAP];
            float4 _EnhancedWarmLightColors[ENHANCED_WARM_LIGHT_CAP];
            float4 _EnhancedWarmLightRanges[ENHANCED_WARM_LIGHT_CAP];
            int _EnhancedWarmLightCount;

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
                half specular = pow(saturate(dot(normalWS, halfDirection)), 32.0) * _SpecularStrength;
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

            half3 EvaluateWarmLight(float3 positionWS, half3 normalWS)
            {
                half3 contribution = 0.0h;
                int count = min(_EnhancedWarmLightCount, ENHANCED_WARM_LIGHT_CAP);
                for (int index = 0; index < count; index++)
                {
                    float3 delta = _EnhancedWarmLightPositions[index].xyz - positionWS;
                    float distanceToLight = length(delta);
                    float range = max(_EnhancedWarmLightRanges[index].x, 0.001);
                    half attenuation = saturate(1.0h - distanceToLight / range);
                    attenuation *= attenuation;
                    half3 direction = SafeNormalize(delta);
                    half facing = saturate(dot(normalWS, direction));
                    half3 lightColor = _EnhancedWarmLightColors[index].rgb;
                    half intensity = _EnhancedWarmLightColors[index].a;
                    half shimmer = 0.82h + 0.18h * sin(
                        _Time.y * 3.6h + dot(positionWS.xz, float2(0.61, -0.37)));
                    contribution += lightColor * intensity * attenuation *
                        (facing * 0.38h + shimmer * 0.16h);
                }
                return contribution;
            }

            half4 Frag(Varyings input) : SV_Target
            {
                half3 normalWS = NormalizeNormalPerPixel(input.normalWS);
                half3 viewDirectionWS = GetWorldSpaceNormalizeViewDir(input.positionWS);
                half3 baseColor = saturate(input.color.rgb * _BaseColor.rgb);
                float macroDetail = ValueNoise(input.positionWS.xz * _DetailScale);
                float fineDetail = ValueNoise(input.positionWS.xz * (_DetailScale * 3.5));
                float detail = lerp(macroDetail, fineDetail, 0.32);
                // The material is deliberately texture-free, but it should
                // still break up the generated terrain at game distance.
                // Keep the modulation bounded so this remains a presentation
                // treatment rather than a second terrain authority.
                half detailFactor = lerp(0.72h, 1.28h, detail);
                baseColor *= lerp(1.0h, detailFactor, _DetailStrength);
                // Steep generated faces receive a restrained blue moonlit
                // tint. This adds readable relief to the low-poly terrain
                // without baking a normal or albedo texture.
                half slope = saturate(1.0h - normalWS.y);
                baseColor *= lerp(
                    1.0h,
                    half3(0.72h, 0.84h, 1.08h),
                    slope * 0.22h);
                // Keep shadowed surfaces readable without flattening the night
                // palette into neutral gray. Warm emissive families and local
                // spell lights remain distinct against this cool fill.
                half3 ambientFloor = _AmbientBoost * half3(0.68h, 0.80h, 1.0h);
                half3 lighting = max(SampleSH(normalWS), ambientFloor);

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
                // Town/castle meshes mark doors and trim with a brighter
                // vertex color. Give those authored structural accents a
                // small emissive cue without replacing the owner geometry.
                half accentMask = smoothstep(0.76h, 0.94h, input.color.g) *
                                  smoothstep(0.82h, 0.99h, input.color.r);
                color += _AccentColor.rgb * (_AccentStrength * accentMask);
                half3 warmLighting = EvaluateWarmLight(input.positionWS, normalWS);
                color += baseColor * warmLighting * 0.72h + warmLighting * 0.08h;
                half rim = pow(saturate(1.0h - dot(normalWS, viewDirectionWS)), 2.0h);
                color += _RimColor.rgb * (rim * _RimStrength);
                color += _EmissionColor.rgb * _EmissionStrength;
                color = MixFog(color, ComputeFogFactor(input.positionHCS.z));
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
