Shader "MagicCarpet/Enhanced/Impact"
{
    Properties
    {
        [HDR] _GlowColor ("Glow Color", Color) = (1, 0.84, 0.18, 1)
        [HDR] _ShockwaveColor ("Shockwave Color", Color) = (1, 0.38, 0.04, 1)
        _GlowIntensity ("Glow Intensity", Range(0, 8)) = 2.8
        _ShockwaveStrength ("Shockwave Strength", Range(0, 8)) = 2.4
        _SparkStrength ("Spark Strength", Range(0, 8)) = 1.5
        _Opacity ("Opacity", Range(0, 1)) = 0.92
        _PulseSpeed ("Pulse Speed", Float) = 5
        _RimStrength ("Rim Strength", Range(0, 4)) = 1.8
        _ImpactProgress ("Impact Progress", Range(0, 1)) = 0
    }

    SubShader
    {
        Tags
        {
            "Queue" = "Transparent+30"
            "RenderType" = "Transparent"
            "RenderPipeline" = "UniversalPipeline"
        }

        Blend SrcAlpha One
        ZWrite Off
        // An impact is a transient magical readability cue. It remains bound
        // to the authoritative world position, but its additive shimmer must
        // not disappear behind the exact terrain triangle it illuminates.
        ZTest Always
        Cull Off

        Pass
        {
            Name "ImpactGlow"
            Tags { "LightMode" = "UniversalForward" }

            HLSLPROGRAM
            #pragma vertex Vert
            #pragma fragment Frag

            #include "Packages/com.unity.render-pipelines.universal/ShaderLibrary/Core.hlsl"

            CBUFFER_START(UnityPerMaterial)
                half4 _GlowColor;
                half4 _ShockwaveColor;
                half _GlowIntensity;
                half _ShockwaveStrength;
                half _SparkStrength;
                half _Opacity;
                half _PulseSpeed;
                half _RimStrength;
                half _ImpactProgress;
            CBUFFER_END

            struct Attributes
            {
                float4 positionOS : POSITION;
                float3 normalOS : NORMAL;
                float2 uv : TEXCOORD0;
            };

            struct Varyings
            {
                float4 positionHCS : SV_POSITION;
                float3 positionWS : TEXCOORD0;
                half3 normalWS : TEXCOORD1;
                half2 uv : TEXCOORD2;
            };

            Varyings Vert(Attributes input)
            {
                Varyings output;
                VertexPositionInputs positionInputs = GetVertexPositionInputs(input.positionOS.xyz);
                VertexNormalInputs normalInputs = GetVertexNormalInputs(input.normalOS);
                output.positionHCS = positionInputs.positionCS;
                output.positionWS = positionInputs.positionWS;
                output.normalWS = normalInputs.normalWS;
                output.uv = input.uv;
                return output;
            }

            half4 Frag(Varyings input) : SV_Target
            {
                half3 normalWS = NormalizeNormalPerPixel(input.normalWS);
                half3 viewDirectionWS = GetWorldSpaceNormalizeViewDir(input.positionWS);
                half rim = pow(saturate(1.0h - dot(normalWS, viewDirectionWS)), 2.0h);
                half pulse = 0.80h + 0.20h * sin(_Time.y * _PulseSpeed);
                half intensity = _GlowIntensity * pulse * (0.78h + rim * _RimStrength);
                half alpha = saturate(_Opacity * (0.80h + rim * 0.20h));
                half2 centered = input.uv * 2.0h - 1.0h;
                half radius = length(centered);
                half edgeFade = 1.0h - smoothstep(0.78h, 1.0h, radius);
                half progress = saturate(_ImpactProgress);
                half waveRadius = lerp(0.28h, 0.92h, progress);
                half waveWidth = lerp(0.065h, 0.12h, progress);
                half shockwaveBand = 1.0h - smoothstep(
                    0.0h,
                    waveWidth,
                    abs(radius - waveRadius));
                half angle = atan2(centered.y, centered.x);
                half sparkPattern = pow(saturate(0.5h + 0.5h * cos(
                    angle * 8.0h + _Time.y * (_PulseSpeed * 1.8h))), 9.0h);
                half shockwave = _ShockwaveStrength * shockwaveBand * edgeFade *
                                 (1.0h - progress * 0.45h);
                half sparks = _SparkStrength * sparkPattern * edgeFade *
                              (0.35h + rim * 0.65h) * (1.0h - progress * 0.5h);
                alpha = saturate(alpha + shockwaveBand * 0.28h + sparks * 0.12h);
                half3 color = _GlowColor.rgb * intensity;
                color += _ShockwaveColor.rgb * shockwave;
                color += _GlowColor.rgb * sparks;
                return half4(color, alpha);
            }
            ENDHLSL
        }
    }
}
