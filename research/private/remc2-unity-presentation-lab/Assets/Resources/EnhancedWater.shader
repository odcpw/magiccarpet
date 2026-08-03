Shader "MagicCarpet/Enhanced/Water"
{
    Properties
    {
        [MainColor] _BaseColor ("Base Color", Color) = (0.008, 0.04, 0.12, 1)
        _ReflectionColor ("Sky Reflection", Color) = (0.025, 0.15, 0.42, 1)
        _FoamColor ("Wave Crest", Color) = (0.08, 0.25, 0.60, 1)
        [HDR] _MoonColor ("Moon Streak Color", Color) = (0.34, 0.58, 1, 1)
        _WaveScale ("Wave Scale", Float) = 0.085
        _WaveSpeed ("Wave Speed", Float) = 0.32
        _WaveStrength ("Wave Normal Strength", Range(0, 1)) = 0.28
        _FresnelStrength ("Fresnel Strength", Range(0, 2)) = 1.30
        _SpecularStrength ("Moon Glint Strength", Range(0, 4)) = 1.75
        _ReflectionStrength ("Reflection Strength", Range(0, 2)) = 1.15
        _SceneReflectionStrength ("Scene Reflection", Range(0, 1)) = 0.72
        _ReflectionDistortion ("Reflection Distortion", Range(0, 0.2)) = 0.035
        _PlanarReflectionEnabled ("Planar Reflection Enabled", Float) = 0
        _PlanarReflectionMix ("Planar Reflection Mix", Range(0, 1)) = 0.46
        _PlanarReflectionTexture ("Planar Reflection", 2D) = "black" {}
        _CausticColor ("Caustic Shimmer", Color) = (0.18, 0.65, 1, 1)
        _CausticScale ("Caustic Scale", Float) = 0.22
        _CausticStrength ("Caustic Strength", Range(0, 2)) = 0.9
        _RippleCenter ("Ripple Center", Vector) = (0, 0, 0, 0)
        _RippleStartTime ("Ripple Start Time", Float) = 0
        _RippleStrength ("Ripple Strength", Range(0, 2)) = 0
        _RippleSpeed ("Ripple Speed", Float) = 2.8
        _RippleWidth ("Ripple Width", Float) = 0.65
        _Alpha ("Surface Opacity", Range(0, 1)) = 0.94
    }

    SubShader
    {
        Tags
        {
            "Queue" = "Transparent"
            "RenderType" = "Transparent"
            "RenderPipeline" = "UniversalPipeline"
        }

        Blend SrcAlpha OneMinusSrcAlpha
        ZWrite Off
        Cull Back

        Pass
        {
            Name "WaterForward"
            Tags { "LightMode" = "UniversalForward" }

            HLSLPROGRAM
            #pragma vertex Vert
            #pragma fragment Frag
            #pragma multi_compile _ _ADDITIONAL_LIGHTS
            #pragma multi_compile_fog

            #include "Packages/com.unity.render-pipelines.universal/ShaderLibrary/Core.hlsl"
            #include "Packages/com.unity.render-pipelines.universal/ShaderLibrary/DeclareOpaqueTexture.hlsl"
            #include "Packages/com.unity.render-pipelines.universal/ShaderLibrary/Lighting.hlsl"

            CBUFFER_START(UnityPerMaterial)
                half4 _BaseColor;
                half4 _ReflectionColor;
                half4 _FoamColor;
                half4 _MoonColor;
                half _WaveScale;
                half _WaveSpeed;
                half _WaveStrength;
                half _FresnelStrength;
                half _SpecularStrength;
                half _ReflectionStrength;
                half _SceneReflectionStrength;
                half _ReflectionDistortion;
                half _PlanarReflectionEnabled;
                half _PlanarReflectionMix;
                half4 _CausticColor;
                half _CausticScale;
                half _CausticStrength;
                float4 _RippleCenter;
                half _RippleStartTime;
                half _RippleStrength;
                half _RippleSpeed;
                half _RippleWidth;
                half _Alpha;
            CBUFFER_END

            TEXTURE2D(_PlanarReflectionTexture);
            SAMPLER(sampler_PlanarReflectionTexture);

            #define ENHANCED_WARM_LIGHT_CAP 4
            float4 _EnhancedWarmLightPositions[ENHANCED_WARM_LIGHT_CAP];
            float4 _EnhancedWarmLightColors[ENHANCED_WARM_LIGHT_CAP];
            float4 _EnhancedWarmLightRanges[ENHANCED_WARM_LIGHT_CAP];
            int _EnhancedWarmLightCount;

            struct Attributes
            {
                float4 positionOS : POSITION;
                float3 normalOS : NORMAL;
            };

            struct Varyings
            {
                float4 positionHCS : SV_POSITION;
                float3 positionWS : TEXCOORD0;
                half3 normalWS : TEXCOORD1;
                float4 screenPos : TEXCOORD2;
            };

            Varyings Vert(Attributes input)
            {
                Varyings output;
                VertexPositionInputs positionInputs = GetVertexPositionInputs(input.positionOS.xyz);
                VertexNormalInputs normalInputs = GetVertexNormalInputs(input.normalOS);
                output.positionHCS = positionInputs.positionCS;
                output.positionWS = positionInputs.positionWS;
                output.normalWS = normalInputs.normalWS;
                output.screenPos = ComputeScreenPos(output.positionHCS);
                return output;
            }

            float LayerPhase(float2 position, float time, float seed)
            {
                float broad = sin(dot(position, float2(0.37, 0.61)) + time * 0.13 + seed);
                float fine = cos(dot(position, float2(-0.53, 0.29)) - time * 0.09 - seed * 1.37);
                return broad * 0.22 + fine * 0.11;
            }

            float Wave(float2 position, float2 direction, float scale, float time, float seed)
            {
                return sin(dot(position * scale, direction) + time + LayerPhase(position, time, seed));
            }

            float2 WaveSlope(float2 position, float2 direction, float scale, float time, float seed)
            {
                float epsilon = 0.14;
                float center = Wave(position, direction, scale, time, seed);
                float x = Wave(position + float2(epsilon, 0.0), direction, scale, time, seed);
                float z = Wave(position + float2(0.0, epsilon), direction, scale, time, seed);
                return float2((x - center) / epsilon, (z - center) / epsilon);
            }

            half CausticPattern(float2 position)
            {
                float time = _Time.y;
                float2 coordinate = position * max(_CausticScale, 0.001h);
                half waveA = sin(dot(coordinate, float2(1.17, 0.63)) + time * 0.42h);
                half waveB = sin(dot(coordinate, float2(-0.71, 1.31)) - time * 0.31h + 1.7h);
                half waveC = sin(dot(coordinate, float2(0.37, -1.09)) + time * 0.19h - 0.8h);
                half interference = 0.5h + 0.5h * (waveA * waveB * 0.72h + waveC * 0.28h);
                return smoothstep(0.58h, 0.92h, interference);
            }

            half3 WaveNormal(float2 position, half3 baseNormal)
            {
                float time = _Time.y;
                float2 directionA = normalize(float2(1.0, 0.35));
                float2 directionB = normalize(float2(-0.25, 1.0));
                float2 directionC = normalize(float2(0.73, -0.91));
                float2 slopeA = WaveSlope(position, directionA, _WaveScale, time * _WaveSpeed, 0.0);
                float2 slopeB = WaveSlope(position, directionB, _WaveScale * 1.85, time * _WaveSpeed * 1.35, 1.7);
                float2 slopeC = WaveSlope(position, directionC, _WaveScale * 3.6, time * _WaveSpeed * 0.73, -2.2);
                float2 slope = slopeA * 0.52 + slopeB * 0.30 + slopeC * 0.18;

                float2 rippleDelta = position - _RippleCenter.xz;
                float rippleDistance = length(rippleDelta);
                float rippleAge = max(0.0, _Time.y - _RippleStartTime);
                float rippleRadius = rippleAge * _RippleSpeed;
                float rippleEnvelope = saturate(1.0 - rippleAge / 5.0);
                float rippleBand = exp(-pow((rippleDistance - rippleRadius) / max(0.05, _RippleWidth), 2.0)) *
                                   rippleEnvelope * _RippleStrength;
                float2 rippleDirection = rippleDistance > 0.001
                    ? rippleDelta / rippleDistance
                    : float2(0.0, 0.0);
                slope += rippleDirection * rippleBand * 1.8;
                return normalize(baseNormal + half3(slope.x, 0.0h, slope.y) * _WaveStrength);
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
                    half3 color = _EnhancedWarmLightColors[index].rgb;
                    half intensity = _EnhancedWarmLightColors[index].a;
                    half shimmer = 0.70h + 0.30h * sin(
                        _Time.y * 3.1h + dot(positionWS.xz, float2(0.55, -0.43)));
                    half shell = exp(-pow(
                        (distanceToLight - range * 0.40h) / max(range * 0.20h, 0.05h),
                        2.0h));
                    contribution += color * intensity * attenuation *
                        (facing * 0.24h + shimmer * 0.24h) +
                        color * intensity * shell * (0.08h + shimmer * 0.08h);
                }
                return contribution;
            }

            half3 EvaluateAdditionalLight(Light light, half3 normalWS, half3 viewDirectionWS)
            {
                half diffuse = saturate(dot(normalWS, light.direction));
                half3 halfDirection = SafeNormalize(light.direction + viewDirectionWS);
                half specular = pow(saturate(dot(normalWS, halfDirection)), 32.0h) * 0.55h;
                half attenuation = light.distanceAttenuation * light.shadowAttenuation;
                return (diffuse.xxx * 0.45h + specular.xxx) * light.color * attenuation;
            }

            half4 Frag(Varyings input) : SV_Target
            {
                half3 normalWS = WaveNormal(input.positionWS.xz, NormalizeNormalPerPixel(input.normalWS));
                half3 viewDirectionWS = GetWorldSpaceNormalizeViewDir(input.positionWS);
                half fresnel = pow(1.0h - saturate(dot(normalWS, viewDirectionWS)), 2.2h);
                float layerA = Wave(input.positionWS.xz, normalize(float2(1.0, 0.35)), _WaveScale, _Time.y * _WaveSpeed, 0.0);
                float layerB = Wave(input.positionWS.xz, normalize(float2(-0.25, 1.0)), _WaveScale * 1.85, _Time.y * _WaveSpeed * 1.35, 1.7);
                float layerC = Wave(input.positionWS.xz, normalize(float2(0.73, -0.91)), _WaveScale * 3.6, _Time.y * _WaveSpeed * 0.73, -2.2);
                half crest = smoothstep(0.70h, 0.93h, layerA * 0.52h + layerB * 0.30h + layerC * 0.18h);

                Light moon = GetMainLight();
                half3 halfDirection = SafeNormalize(moon.direction + viewDirectionWS);
                half glint = pow(saturate(dot(normalWS, halfDirection)), 24.0h) *
                             moon.distanceAttenuation * _SpecularStrength;
                float2 screenUV = input.screenPos.xy / max(input.screenPos.w, 0.0001);
                float2 reflectionUV = float2(screenUV.x, 1.0 - screenUV.y);
                reflectionUV = saturate(reflectionUV + normalWS.xz * _ReflectionDistortion);
                float2 planarReflectionUV = saturate(screenUV + normalWS.xz * _ReflectionDistortion);
                half3 sceneReflection = SampleSceneColor(reflectionUV);
                half3 planarReflection = SAMPLE_TEXTURE2D(
                    _PlanarReflectionTexture,
                    sampler_PlanarReflectionTexture,
                    planarReflectionUV).rgb;
                // Keep the screen-space fallback as the readable base layer.
                // On some URP/OpenGL paths the bounded reflection camera sees
                // mostly the dark horizon; replacing the fallback wholesale
                // makes the live water look like a black card. A controlled
                // mix preserves the existing color while adding real mirrored
                // terrain/light structure when the planar pass is useful.
                half planarMix = saturate(_PlanarReflectionEnabled) *
                                 saturate(_PlanarReflectionMix);
                half3 resolvedSceneReflection = lerp(
                    sceneReflection,
                    planarReflection,
                    planarMix);
                half3 reflection = lerp(
                    _ReflectionColor.rgb,
                    resolvedSceneReflection,
                    _SceneReflectionStrength);
                reflection *= _ReflectionStrength * (0.35h + fresnel * _FresnelStrength);
                half3 color = lerp(_BaseColor.rgb, reflection, saturate(0.32h + fresnel * 0.72h));

                // The ripple normal above makes the surface react physically;
                // this narrow moving band makes a real owner splash readable
                // at game distance without adding a second gameplay surface.
                float2 rippleDelta = input.positionWS.xz - _RippleCenter.xz;
                float rippleDistance = length(rippleDelta);
                float rippleAge = max(0.0, _Time.y - _RippleStartTime);
                float rippleRadius = rippleAge * _RippleSpeed;
                float rippleEnvelope = saturate(1.0 - rippleAge / 5.0);
                half rippleBand = exp(-pow(
                    (rippleDistance - rippleRadius) / max(0.05, _RippleWidth),
                    2.0)) * rippleEnvelope * _RippleStrength;

                half3 additionalLighting = 0.0h;
                #if defined(_ADDITIONAL_LIGHTS)
                    uint additionalLightCount = GetAdditionalLightsCount();
                    for (uint index = 0u; index < additionalLightCount; index++)
                    {
                        additionalLighting += EvaluateAdditionalLight(
                            GetAdditionalLight(index, input.positionWS),
                            normalWS,
                            viewDirectionWS);
                    }
                #endif
                half3 warmLighting = EvaluateWarmLight(input.positionWS, normalWS);
                half warmShimmer = 0.55h + 0.45h * sin(
                    _Time.y * 3.3h + dot(input.positionWS.xz, float2(0.58, -0.41)));
                half caustic = CausticPattern(input.positionWS.xz);
                color += additionalLighting * (0.08h + fresnel * 0.18h);
                color += warmLighting * (0.25h + fresnel * 0.45h + warmShimmer * (0.18h + crest * 0.08h));
                color += _CausticColor.rgb * caustic * _CausticStrength *
                         (0.025h + fresnel * 0.11h);
                color += _FoamColor.rgb * crest * (0.018h + fresnel * 0.05h);
                color += _FoamColor.rgb * rippleBand * (0.16h + fresnel * 0.58h);
                color += _MoonColor.rgb * rippleBand * (0.06h + crest * 0.12h);
                color += _MoonColor.rgb * glint;
                color += _MoonColor.rgb * fresnel * crest * 0.035h;
                color = MixFog(color, ComputeFogFactor(input.positionHCS.z));
                return half4(color, _Alpha);
            }
            ENDHLSL
        }
    }
}
