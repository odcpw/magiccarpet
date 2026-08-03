Shader "MagicCarpet/Enhanced/Terrain"
{
    Properties
    {
        [MainColor] _BaseColor ("Base Color", Color) = (1, 1, 1, 1)
        _DetailScale ("Procedural Detail Scale", Float) = 0.075
        _DetailStrength ("Procedural Detail Strength", Range(0, 1)) = 0.42
        _MicroDetailScale ("Near Surface Detail Scale", Float) = 0.62
        _MicroDetailStrength ("Near Surface Detail Strength", Range(0, 1)) = 0.28
        _NearDetailSharpenStrength ("Near Detail Sharpen Strength", Range(0, 0.5)) = 0.36
        _MicroReliefStrength ("Near Surface Relief Strength", Range(0, 1)) = 0.20
        _MicroDetailNearStart ("Near Surface Detail Start", Float) = 8
        _MicroDetailNearEnd ("Near Surface Detail End", Float) = 128
        _SlopeTint ("Moonlit Slope Tint", Color) = (0.72, 0.84, 1.08, 1)
        _SlopeStrength ("Slope Tint Strength", Range(0, 1)) = 0.42
        _AmbientBoost ("Night Ambient Boost", Range(0, 2)) = 0.65
        _SpecularStrength ("Moon Specular Strength", Range(0, 1)) = 0.10
        _NormalStrength ("Procedural Relief Strength", Range(0, 1)) = 0.16
        _RidgeStrength ("Procedural Ridge Strength", Range(0, 1)) = 0.24
        _RockTint ("Rock Slope Tint", Color) = (0.62, 0.72, 0.80, 1)
        _SoilTint ("Soil Shelf Tint", Color) = (0.68, 0.56, 0.42, 1)
        _EnhancedAerialStart ("Aerial Perspective Start", Float) = 48
        _EnhancedAerialEnd ("Aerial Perspective End", Float) = 240
        _EnhancedAerialMaxBlend ("Aerial Perspective Max Blend", Range(0, 1)) = 0.68
        _EnhancedAerialColor ("Aerial Perspective Color", Color) = (0.045, 0.12, 0.32, 1)
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
                half _MicroDetailScale;
                half _MicroDetailStrength;
                half _NearDetailSharpenStrength;
                half _MicroReliefStrength;
                half _MicroDetailNearStart;
                half _MicroDetailNearEnd;
                half4 _SlopeTint;
                half _SlopeStrength;
                half _AmbientBoost;
                half _SpecularStrength;
                half _NormalStrength;
                half _RidgeStrength;
                half4 _RockTint;
                half4 _SoilTint;
                half _EnhancedAerialStart;
                half _EnhancedAerialEnd;
                half _EnhancedAerialMaxBlend;
                half4 _EnhancedAerialColor;
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
                half specular = pow(saturate(dot(normalWS, halfDirection)), 32.0h) * _SpecularStrength * 1.8h;
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

            float Fbm(float2 position)
            {
                float octave0 = ValueNoise(position);
                float octave1 = ValueNoise(position * 2.07 + float2(13.1, -4.7));
                float octave2 = ValueNoise(position * 4.19 + float2(-7.3, 19.4));
                float octave3 = ValueNoise(position * 8.41 + float2(31.6, 8.2));
                return (octave0 * 0.50 + octave1 * 0.25 + octave2 * 0.125 + octave3 * 0.0625) / 0.9375;
            }

            float NearSurfaceFbm(float2 position)
            {
                float octave0 = ValueNoise(position);
                float octave1 = ValueNoise(position * 2.03 + float2(-17.4, 6.2));
                float octave2 = ValueNoise(position * 4.17 + float2(8.6, 21.7));
                return saturate(octave0 * 0.56 + octave1 * 0.29 + octave2 * 0.15);
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
                    half shimmer = 0.84h + 0.16h * sin(
                        _Time.y * 3.4h + dot(positionWS.xz, float2(0.61, -0.37)));
                    // A narrow shell keeps a moving spell legible across
                    // nearby faces even when the face points away from the
                    // point light. It is a bounded screen-facing light cue,
                    // not a second gameplay light or a global tint.
                    half shell = exp(-pow(
                        (distanceToLight - range * 0.42h) / max(range * 0.18h, 0.05h),
                        2.0h));
                    contribution += color * intensity * attenuation *
                        (facing * 0.36h + shimmer * 0.18h) +
                        color * intensity * shell * (0.06h + shimmer * 0.06h);
                }
                return contribution;
            }

            half3 ApplyEnhancedAerialPerspective(half3 color, float3 positionWS)
            {
                float distanceFromCamera = distance(GetCameraPositionWS(), positionWS);
                half depth = smoothstep(
                    _EnhancedAerialStart,
                    _EnhancedAerialEnd,
                    distanceFromCamera);
                half blend = depth * min(saturate(_EnhancedAerialMaxBlend), 0.68h);
                return lerp(color, _EnhancedAerialColor.rgb, blend);
            }

            half4 Frag(Varyings input) : SV_Target
            {
                half3 normalWS = NormalizeNormalPerPixel(input.normalWS);
                half3 viewDirectionWS = GetWorldSpaceNormalizeViewDir(input.positionWS);
                half3 baseColor = saturate(input.color.rgb * _BaseColor.rgb);

                float2 worldCoord = input.positionWS.xz * max(_DetailScale, 0.001);
                float broadFbm = Fbm(worldCoord);
                float fineFbm = Fbm(worldCoord * 2.15 + float2(4.7, -9.3));
                float detail = lerp(broadFbm, fineFbm, 0.30);
                float distanceFromCamera = distance(GetCameraPositionWS(), input.positionWS);
                half nearSurfaceFade = 1.0h - smoothstep(
                    _MicroDetailNearStart,
                    max(_MicroDetailNearEnd, _MicroDetailNearStart + 0.001h),
                    distanceFromCamera);

                // Whole Hog needs a readable surface material even when the
                // owner selector plane contains broad, low-frequency color
                // blocks. This bounded second scale adds soil/rock breakup in
                // the foreground and fades before it can become distant
                // shimmer. It changes only presentation shading, never the
                // authoritative terrain or its selector identity.
                float2 microCoord = input.positionWS.xz * max(_MicroDetailScale, 0.001);
                float microDetail = NearSurfaceFbm(microCoord);
                float2 microReliefCoord = input.positionWS.xz * max(_MicroDetailScale * 1.45, 0.001);
                const float microReliefStep = 0.065;
                float microLeft = NearSurfaceFbm(microReliefCoord - float2(microReliefStep, 0.0));
                float microRight = NearSurfaceFbm(microReliefCoord + float2(microReliefStep, 0.0));
                float microDown = NearSurfaceFbm(microReliefCoord - float2(0.0, microReliefStep));
                float microUp = NearSurfaceFbm(microReliefCoord + float2(0.0, microReliefStep));
                float2 microGradient = float2(microRight - microLeft, microUp - microDown);
                float2 grainCoord = input.positionWS.xz * max(_MicroDetailScale * 2.4, 0.001);
                float grain = NearSurfaceFbm(grainCoord + float2(17.4, -6.8));

                // The terrain mesh is intentionally asset-free.  Reuse the
                // multi-octave scalar field above as a bounded normal perturbation
                // so moonlight breaks across broad relief instead of only
                // changing the albedo.  The finite-difference step is large
                // enough to avoid noisy pixel-scale shimmer at game scale.
                float2 reliefCoord = input.positionWS.xz * max(_DetailScale * 2.4, 0.001);
                const float reliefStep = 0.12;
                float reliefLeft = Fbm(reliefCoord - float2(reliefStep, 0.0));
                float reliefRight = Fbm(reliefCoord + float2(reliefStep, 0.0));
                float reliefDown = Fbm(reliefCoord - float2(0.0, reliefStep));
                float reliefUp = Fbm(reliefCoord + float2(0.0, reliefStep));
                float2 reliefGradient = float2(reliefRight - reliefLeft, reliefUp - reliefDown);
                float2 fineReliefCoord = input.positionWS.xz * max(_DetailScale * 6.5, 0.001);
                const float fineReliefStep = 0.055;
                float fineLeft = Fbm(fineReliefCoord - float2(fineReliefStep, 0.0));
                float fineRight = Fbm(fineReliefCoord + float2(fineReliefStep, 0.0));
                float fineDown = Fbm(fineReliefCoord - float2(0.0, fineReliefStep));
                float fineUp = Fbm(fineReliefCoord + float2(0.0, fineReliefStep));
                float2 fineGradient = float2(fineRight - fineLeft, fineUp - fineDown);
                reliefGradient = reliefGradient * 0.78 + fineGradient * 0.22;
                normalWS = SafeNormalize(normalWS + half3(
                    -reliefGradient.x * _NormalStrength * 2.10,
                    0.0h,
                    -reliefGradient.y * _NormalStrength * 2.10));
                normalWS = SafeNormalize(normalWS + half3(
                    -microGradient.x * _MicroReliefStrength * nearSurfaceFade * 1.55,
                    0.0h,
                    -microGradient.y * _MicroReliefStrength * nearSurfaceFade * 1.55));

                half variation = lerp(0.50h, 1.55h, saturate(detail));
                baseColor *= lerp(1.0h, variation, _DetailStrength);

                half slope = saturate(1.0h - normalWS.y);
                half exposedFace = smoothstep(0.18h, 0.90h, slope);
                half landMask = smoothstep(0.08h, 0.18h, input.color.r);
                half nearSurfaceMask = nearSurfaceFade * saturate(
                    landMask * (0.62h + slope * 0.58h));
                half nearDetailHighPass = clamp((half)(grain - microDetail), -0.30h, 0.30h);
                baseColor *= 1.0h + nearDetailHighPass * _NearDetailSharpenStrength * nearSurfaceMask;
                half microContrast = (half)(microDetail * 2.0 - 1.0);
                half3 microTint = lerp(
                    _SoilTint.rgb * 0.72h,
                    _RockTint.rgb * 1.04h,
                    smoothstep(0.38h, 0.76h, microDetail));
                baseColor *= 1.0h + microContrast * _MicroDetailStrength * nearSurfaceMask;
                baseColor = lerp(
                    baseColor,
                    baseColor * (0.90h + microTint * 0.20h),
                    nearSurfaceMask * 0.24h);
                baseColor = lerp(
                    baseColor,
                    baseColor * _SlopeTint.rgb,
                    exposedFace * _SlopeStrength);

                half rockMask = smoothstep(0.34h, 0.82h, slope) * landMask;
                half soilMask = smoothstep(0.16h, 0.54h, 1.0h - slope) * landMask;
                half3 rockSoilTint = lerp(_SoilTint.rgb, _RockTint.rgb, rockMask);
                baseColor = lerp(
                    baseColor,
                    baseColor * (0.68h + rockSoilTint * 0.78h) + rockSoilTint * 0.032h,
                    saturate((rockMask + soilMask) * 0.44h));

                // A second, smaller procedural scale supplies the missing
                // high-resolution material read on the generated terrain.
                // It is strongest on nearby land and exposed faces, then
                // fades with distance so the night scene remains calm rather
                // than turning into noisy concept-art texture.
                half3 grainTint = lerp(
                    _SoilTint.rgb * 0.72h,
                    _RockTint.rgb * 1.10h,
                    grain);
                half grainMask = nearSurfaceMask *
                                  saturate(0.25h + slope * 0.55h);
                baseColor = lerp(
                    baseColor,
                    baseColor * saturate(0.72h + grainTint * 0.55h),
                    grainMask * 0.42h);

                // A soft ridge mask adds readable strata to the low-poly
                // surface while remaining a single material pass.  It is
                // deliberately restrained so the night palette remains cool
                // and the original terrain selectors stay legible.
                float strataPhase = input.positionWS.y * 0.42 +
                                    dot(input.positionWS.xz, normalize(float2(0.86, 0.49))) * 0.055 +
                                    detail * 1.8;
                half strataMask = smoothstep(0.76h, 0.97h, abs(sin(strataPhase)));
                half ridgeMask = smoothstep(0.66h, 0.94h, saturate(detail));
                ridgeMask = max(ridgeMask, strataMask * 0.52h);
                ridgeMask *= saturate(0.12h + slope * 1.20h) * landMask;
                half3 ridgeTint = lerp(
                    _SoilTint.rgb * 0.72h,
                    _RockTint.rgb * 1.06h,
                    saturate(detail * 1.4h));
                baseColor = lerp(
                    baseColor,
                    baseColor * (0.72h + ridgeTint * 0.55h) + ridgeTint * 0.028h,
                    ridgeMask * _RidgeStrength);

                // Let steep authoritative faces fall below the broad night
                // ambient floor so the existing moon direction can describe
                // relief instead of flattening every slab to one value.  The
                // reduction is capped at 28 percent and is presentation-only.
                half reliefFace = max(
                    exposedFace,
                    smoothstep(0.04h, 0.48h, slope));
                half ambientRelief = saturate(reliefFace * _SlopeStrength * 0.28h);
                half3 ambientFloor = _AmbientBoost * half3(0.60h, 0.73h, 0.96h) *
                                     (1.0h - ambientRelief);
                half3 lighting = max(
                    SampleSH(normalWS) * (1.0h - ambientRelief),
                    ambientFloor);
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

                half3 warmLighting = EvaluateWarmLight(input.positionWS, normalWS);
                half3 color = baseColor * (lighting + warmLighting * 1.0h);
                color += warmLighting * 0.14h;
                half rim = pow(saturate(1.0h - dot(normalWS, viewDirectionWS)), 2.0h);
                color += _SlopeTint.rgb * (rim * exposedFace * 0.08h);
                color = ApplyEnhancedAerialPerspective(color, input.positionWS);
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
