Shader "MagicCarpet/Enhanced/Fireball"
{
    Properties
    {
        [HDR] _CoreColor ("Hot Core Color", Color) = (1, 0.72, 0.22, 1)
        [HDR] _HaloColor ("Halo Color", Color) = (1, 0.06, 0.008, 1)
        _CoreIntensity ("Core Intensity", Range(0, 12)) = 4.2
        _HaloIntensity ("Halo Intensity", Range(0, 8)) = 1.65
        _CoreRadius ("Core Radius", Range(0.05, 0.8)) = 0.42
        _HaloRadius ("Halo Radius", Range(0.2, 1)) = 0.94
        _OuterRadius ("Outer Glow Radius", Range(0.8, 1.6)) = 1.36
        _OuterIntensity ("Outer Glow Intensity", Range(0, 8)) = 1.8
        _Opacity ("Opacity", Range(0, 1)) = 0.88
        _PulseSpeed ("Pulse Speed", Float) = 2.8
        _ShimmerStrength ("Shimmer Strength", Range(0, 0.4)) = 0.08
    }

    SubShader
    {
        Tags
        {
            "Queue" = "Transparent+20"
            "RenderType" = "Transparent"
            "RenderPipeline" = "UniversalPipeline"
        }

        Blend SrcAlpha One
        ZWrite Off
        Cull Off

        Pass
        {
            Name "FireballGlow"
            Tags { "LightMode" = "UniversalForward" }

            HLSLPROGRAM
            #pragma vertex Vert
            #pragma fragment Frag

            #include "Packages/com.unity.render-pipelines.universal/ShaderLibrary/Core.hlsl"

            CBUFFER_START(UnityPerMaterial)
                half4 _CoreColor;
                half4 _HaloColor;
                half _CoreIntensity;
                half _HaloIntensity;
                half _CoreRadius;
                half _HaloRadius;
                half _OuterRadius;
                half _OuterIntensity;
                half _Opacity;
                half _PulseSpeed;
                half _ShimmerStrength;
            CBUFFER_END

            struct Attributes
            {
                float4 positionOS : POSITION;
                float2 uv : TEXCOORD0;
            };

            struct Varyings
            {
                float4 positionHCS : SV_POSITION;
                half2 uv : TEXCOORD0;
            };

            Varyings Vert(Attributes input)
            {
                Varyings output;
                VertexPositionInputs positionInputs = GetVertexPositionInputs(input.positionOS.xyz);
                output.positionHCS = positionInputs.positionCS;
                output.uv = input.uv;
                return output;
            }

            half4 Frag(Varyings input) : SV_Target
            {
                half2 centered = input.uv * 2.0h - 1.0h;
                half radius = length(centered);
                half core = 1.0h - smoothstep(_CoreRadius * 0.18h, _CoreRadius, radius);
                half halo = saturate(1.0h - radius / _HaloRadius);
                halo *= halo;
                halo = saturate(halo - core * 0.28h);
                half outer = 1.0h - smoothstep(_HaloRadius * 0.55h, _OuterRadius, radius);
                outer *= outer;
                outer = saturate(outer - core * 0.18h);

                half pulse = 0.94h + 0.06h * sin(_Time.y * _PulseSpeed);
                half shimmer = 1.0h + _ShimmerStrength *
                    sin(_Time.y * (_PulseSpeed * 1.7h) + centered.x * 9.0h - centered.y * 7.0h);
                half outerShimmer = 0.84h + 0.16h *
                    sin(_Time.y * (_PulseSpeed * 2.4h) + radius * 18.0h);
                // Break the corona into moving radial tongues.  The result is
                // intentionally soft: bloom supplies the broad light field,
                // while these rays make a cast readable at native resolution.
                half angle = atan2(centered.y, centered.x);
                half ray = pow(saturate(0.5h + 0.5h * cos(
                    angle * 7.0h + _Time.y * (_PulseSpeed * 1.6h))), 9.0h);
                half coronaRing = exp(-pow(
                    (radius - _HaloRadius * 0.76h) / 0.07h, 2.0h));
                half3 color = _CoreColor.rgb * (_CoreIntensity * core) +
                              _HaloColor.rgb * (_HaloIntensity * halo +
                                               _OuterIntensity * outer * outerShimmer +
                                               _OuterIntensity * outer * ray * 0.32h +
                                               _OuterIntensity * coronaRing * 0.16h);
                color *= pulse * shimmer;
                half alpha = saturate(_Opacity * max(max(core, halo * 0.82h), outer * 0.42h));
                return half4(color, alpha);
            }
            ENDHLSL
        }
    }
}
