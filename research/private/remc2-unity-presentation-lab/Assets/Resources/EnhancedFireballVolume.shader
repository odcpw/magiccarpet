Shader "MagicCarpet/Enhanced/FireballVolume"
{
    Properties
    {
        [HDR] _VolumeColor ("Volume Color", Color) = (1, 0.10, 0.012, 1)
        [HDR] _CoreColor ("Core Light Color", Color) = (1, 0.72, 0.16, 1)
        _Intensity ("Volume Intensity", Range(0, 8)) = 2.35
        _CoreIntensity ("Core Intensity", Range(0, 4)) = 1.15
        _Opacity ("Volume Opacity", Range(0, 1)) = 0.52
        _ScrollSpeed ("Scroll Speed", Float) = 2.8
        _NoiseScale ("Noise Scale", Float) = 4.5
        _ShimmerStrength ("Shimmer Strength", Range(0, 1)) = 0.24
    }

    SubShader
    {
        Tags
        {
            "Queue" = "Transparent+10"
            "RenderType" = "Transparent"
            "RenderPipeline" = "UniversalPipeline"
        }

        Blend SrcAlpha One
        ZWrite Off
        Cull Off

        Pass
        {
            Name "FireballVolumeGlow"
            Tags { "LightMode" = "UniversalForward" }

            HLSLPROGRAM
            #pragma vertex Vert
            #pragma fragment Frag

            #include "Packages/com.unity.render-pipelines.universal/ShaderLibrary/Core.hlsl"

            CBUFFER_START(UnityPerMaterial)
                half4 _VolumeColor;
                half4 _CoreColor;
                half _Intensity;
                half _CoreIntensity;
                half _Opacity;
                half _ScrollSpeed;
                half _NoiseScale;
                half _ShimmerStrength;
            CBUFFER_END

            struct Attributes
            {
                float4 positionOS : POSITION;
                float2 uv : TEXCOORD0;
                half4 color : COLOR;
            };

            struct Varyings
            {
                float4 positionHCS : SV_POSITION;
                half2 uv : TEXCOORD0;
                half4 color : COLOR;
            };

            Varyings Vert(Attributes input)
            {
                Varyings output;
                VertexPositionInputs positionInputs = GetVertexPositionInputs(input.positionOS.xyz);
                output.positionHCS = positionInputs.positionCS;
                output.uv = input.uv;
                output.color = input.color;
                return output;
            }

            float Hash21(float2 position)
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
                float lower = lerp(Hash21(cell), Hash21(cell + float2(1.0, 0.0)), fraction.x);
                float upper = lerp(Hash21(cell + float2(0.0, 1.0)), Hash21(cell + 1.0), fraction.x);
                return lerp(lower, upper, fraction.y);
            }

            half4 Frag(Varyings input) : SV_Target
            {
                half along = saturate(input.uv.x);
                half across = saturate(1.0h - abs(input.uv.y * 2.0h - 1.0h));
                half body = smoothstep(0.0h, 0.16h, across) * pow(across, 0.72h);
                half taper = saturate(1.0h - along * 0.92h);
                half softEdge = smoothstep(0.0h, 0.36h, across);
                body *= softEdge * taper;

                float2 noisePosition = input.uv * float2(_NoiseScale * 1.6h, _NoiseScale * 0.45h) +
                                        float2(_Time.y * _ScrollSpeed * 0.12h, -_Time.y * _ScrollSpeed * 0.38h);
                half noise = ValueNoise(noisePosition);
                half fineNoise = ValueNoise(noisePosition * 2.3h + float2(7.1h, -3.4h));
                half wisps = lerp(0.72h, 1.24h, saturate(noise * 0.66h + fineNoise * 0.34h));
                half shimmer = 1.0h + _ShimmerStrength * sin(
                    _Time.y * (_ScrollSpeed * 2.1h) + along * 17.0h + input.uv.y * 9.0h);
                half radialCore = exp(-pow((1.0h - across) / 0.46h, 2.0h)) *
                                  saturate(1.0h - along * 1.65h);
                half ray = pow(saturate(0.5h + 0.5h * cos(
                    along * 23.0h + input.uv.y * 11.0h + _Time.y * _ScrollSpeed)), 4.0h);
                half emission = body * wisps * shimmer * (_Intensity + ray * _Intensity * 0.18h);
                emission += radialCore * _CoreIntensity;
                half alpha = saturate(input.color.a * _Opacity * body * (0.70h + wisps * 0.30h));
                half3 color = lerp(_VolumeColor.rgb, _CoreColor.rgb, radialCore);
                return half4(color * emission, alpha);
            }
            ENDHLSL
        }
    }
}
