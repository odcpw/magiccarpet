Shader "MagicCarpet/Enhanced/NightSky"
{
    Properties
    {
        _ZenithColor ("Zenith Color", Color) = (0.012, 0.035, 0.16, 1)
        _HorizonColor ("Horizon Color", Color) = (0.045, 0.12, 0.32, 1)
        _GroundColor ("Ground Color", Color) = (0.004, 0.012, 0.04, 1)
        _StarColor ("Star Color", Color) = (0.55, 0.72, 1, 1)
        _StarDensity ("Star Density", Range(0, 1)) = 0.025
        _StarIntensity ("Star Intensity", Range(0, 4)) = 0.8
        _StarTwinkleStrength ("Star Twinkle Strength", Range(0, 0.20)) = 0
        _MoonDirection ("Moon Direction", Vector) = (0.35, 0.8, 0.25, 0)
        _MoonColor ("Moon Color", Color) = (0.45, 0.62, 1, 1)
        _MoonIntensity ("Moon Intensity", Range(0, 4)) = 0.8
        _MoonSize ("Moon Size", Range(0.005, 0.2)) = 0.035
        _MoonGlow ("Moon Glow", Range(0, 4)) = 0.45
        _HorizonGlowStrength ("Horizon Glow Strength", Range(0, 0.18)) = 0
        _Exposure ("Exposure", Range(0, 4)) = 1
    }

    SubShader
    {
        Tags
        {
            "Queue" = "Background"
            "RenderType" = "Background"
            "PreviewType" = "Skybox"
            "RenderPipeline" = "UniversalPipeline"
        }

        Cull Off
        ZWrite Off

        Pass
        {
            HLSLPROGRAM
            #pragma vertex Vert
            #pragma fragment Frag

            #include "Packages/com.unity.render-pipelines.universal/ShaderLibrary/Core.hlsl"

            CBUFFER_START(UnityPerMaterial)
                half4 _ZenithColor;
                half4 _HorizonColor;
                half4 _GroundColor;
                half4 _StarColor;
                half _StarDensity;
                half _StarIntensity;
                half _StarTwinkleStrength;
                half4 _MoonDirection;
                half4 _MoonColor;
                half _MoonIntensity;
                half _MoonSize;
                half _MoonGlow;
                half _HorizonGlowStrength;
                half _Exposure;
            CBUFFER_END

            struct Attributes
            {
                float4 positionOS : POSITION;
            };

            struct Varyings
            {
                float4 positionHCS : SV_POSITION;
                float3 direction : TEXCOORD0;
            };

            Varyings Vert(Attributes input)
            {
                Varyings output;
                output.positionHCS = TransformObjectToHClip(input.positionOS.xyz);
                output.direction = input.positionOS.xyz;
                return output;
            }

            float Hash21(float2 value)
            {
                value = frac(value * float2(123.34, 456.21));
                value += dot(value, value + 45.32);
                return frac(value.x * value.y);
            }

            half3 SkyGradient(half3 direction)
            {
                half horizon = smoothstep(0.0h, 0.42h, direction.y);
                half belowHorizon = smoothstep(-0.45h, 0.08h, direction.y);
                half3 upper = lerp(_HorizonColor.rgb, _ZenithColor.rgb, horizon);
                half3 lower = lerp(_GroundColor.rgb, _HorizonColor.rgb, belowHorizon);
                return direction.y >= 0.0h ? upper : lower;
            }

            half Stars(half3 direction)
            {
                half aboveHorizon = smoothstep(0.015h, 0.14h, direction.y);
                float2 spherical = float2(
                    atan2(direction.z, direction.x) * 0.15915494 + 0.5,
                    asin(clamp(direction.y, -1.0, 1.0)) * 0.31830988 + 0.5);
                float2 cell = floor(spherical * float2(180.0, 90.0));
                float2 local = frac(spherical * float2(180.0, 90.0)) - 0.5;
                float starSeed = Hash21(cell);
                half candidate = step(1.0h - _StarDensity, starSeed);
                half radius = smoothstep(0.19h, 0.0h, length(local));
                half twinkle = 1.0h + _StarTwinkleStrength * sin(
                    _Time.y * (1.1h + starSeed * 1.9h) + starSeed * 6.28318h);
                half twinkleHalo = smoothstep(0.30h, 0.0h, length(local)) *
                                   _StarTwinkleStrength * 0.85h;
                return candidate * (radius + twinkleHalo) * aboveHorizon *
                       max(0.0h, twinkle);
            }

            half3 Moon(half3 direction)
            {
                half3 moonDirection = normalize(_MoonDirection.xyz);
                half moonDot = saturate(dot(direction, moonDirection));
                half disc = smoothstep(cos(_MoonSize), cos(_MoonSize * 0.45h), moonDot);
                half glow = pow(moonDot, 48.0h) * _MoonGlow;
                return _MoonColor.rgb * ((disc * _MoonIntensity) + glow);
            }

            half HorizonGlowBand(half3 direction)
            {
                half nearHorizon = smoothstep(-0.02h, 0.035h, direction.y);
                half upperFade = 1.0h - smoothstep(0.12h, 0.34h, direction.y);
                return saturate(nearHorizon * upperFade);
            }

            half4 Frag(Varyings input) : SV_Target
            {
                half3 direction = normalize(input.direction);
                half3 color = SkyGradient(direction);
                half horizonBand = HorizonGlowBand(direction);
                half3 glowColor = saturate(
                    _HorizonColor.rgb * 2.8h + _MoonColor.rgb * 0.15h);
                color += glowColor * horizonBand * _HorizonGlowStrength;
                color += _StarColor.rgb * (Stars(direction) * _StarIntensity);
                color += Moon(direction);
                return half4(max(color * _Exposure, 0.0h), 1.0h);
            }
            ENDHLSL
        }
    }
}
