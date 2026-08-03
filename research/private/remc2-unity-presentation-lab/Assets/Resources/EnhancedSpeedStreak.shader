Shader "MagicCarpet/Enhanced/SpeedStreak"
{
    Properties
    {
        [HDR] _StreakColor ("Streak Color", Color) = (0.18, 0.78, 1, 1)
        [HDR] _CoreColor ("Core Color", Color) = (0.82, 0.98, 1, 1)
        _Intensity ("Intensity", Range(0, 8)) = 2.4
        _Opacity ("Opacity", Range(0, 1)) = 0.62
        _CoreWidth ("Core Width", Range(0.05, 0.8)) = 0.28
        _EdgeSoftness ("Edge Softness", Range(0.02, 0.5)) = 0.18
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
        Cull Off

        Pass
        {
            Name "SpeedStreak"
            Tags { "LightMode" = "UniversalForward" }

            HLSLPROGRAM
            #pragma vertex Vert
            #pragma fragment Frag

            #include "Packages/com.unity.render-pipelines.universal/ShaderLibrary/Core.hlsl"

            CBUFFER_START(UnityPerMaterial)
                half4 _StreakColor;
                half4 _CoreColor;
                half _Intensity;
                half _Opacity;
                half _CoreWidth;
                half _EdgeSoftness;
            CBUFFER_END

            struct Attributes
            {
                float4 positionOS : POSITION;
                float2 uv : TEXCOORD0;
            };

            struct Varyings
            {
                float4 positionHCS : SV_POSITION;
                float2 uv : TEXCOORD0;
            };

            Varyings Vert(Attributes input)
            {
                Varyings output;
                output.positionHCS = TransformObjectToHClip(input.positionOS.xyz);
                output.uv = input.uv;
                return output;
            }

            half4 Frag(Varyings input) : SV_Target
            {
                half across = abs(input.uv.x * 2.0h - 1.0h);
                half edge = 1.0h - smoothstep(
                    saturate(1.0h - _EdgeSoftness),
                    1.0h,
                    across);
                half longitudinal = smoothstep(0.0h, 0.14h, input.uv.y) *
                                    (1.0h - smoothstep(0.82h, 1.0h, input.uv.y));
                half body = edge * longitudinal;
                half core = 1.0h - smoothstep(
                    max(0.01h, _CoreWidth * 0.42h),
                    max(0.02h, _CoreWidth),
                    across);
                half3 color = lerp(_StreakColor.rgb, _CoreColor.rgb, core);
                half intensity = _Intensity * (0.74h + core * 0.46h);
                return half4(color * intensity, body * _Opacity);
            }
            ENDHLSL
        }
    }
}
