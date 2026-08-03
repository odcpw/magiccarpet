Shader "MagicCarpet/Enhanced/FireballTrail"
{
    Properties
    {
        [HDR] _TrailColor ("Trail Color", Color) = (1, 0.16, 0.025, 1)
        _Intensity ("Trail Intensity", Range(0, 12)) = 3.6
        _PulseSpeed ("Pulse Speed", Float) = 3.4
        _ShimmerStrength ("Shimmer Strength", Range(0, 0.4)) = 0.12
        _Opacity ("Opacity", Range(0, 1)) = 0.9
    }

    SubShader
    {
        Tags
        {
            "Queue" = "Transparent+15"
            "RenderType" = "Transparent"
            "RenderPipeline" = "UniversalPipeline"
        }

        Blend SrcAlpha One
        ZWrite Off
        Cull Off

        Pass
        {
            Name "FireballTrailGlow"
            Tags { "LightMode" = "UniversalForward" }

            HLSLPROGRAM
            #pragma vertex Vert
            #pragma fragment Frag

            #include "Packages/com.unity.render-pipelines.universal/ShaderLibrary/Core.hlsl"

            CBUFFER_START(UnityPerMaterial)
                half4 _TrailColor;
                half _Intensity;
                half _PulseSpeed;
                half _ShimmerStrength;
                half _Opacity;
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

            half4 Frag(Varyings input) : SV_Target
            {
                half along = saturate(input.uv.x);
                half across = saturate(1.0h - abs(input.uv.y * 2.0h - 1.0h));
                half edge = smoothstep(0.0h, 0.42h, across) * pow(across, 0.55h);
                half taper = lerp(0.22h, 1.0h, saturate(1.0h - along));
                half pulse = 0.88h + 0.12h * sin(_Time.y * _PulseSpeed + along * 5.0h);
                half shimmer = 1.0h + _ShimmerStrength *
                    sin(_Time.y * (_PulseSpeed * 1.7h) + along * 13.0h);
                half intensity = edge * taper * pulse * shimmer;
                half alpha = saturate(input.color.a * _Opacity * edge * taper);
                half3 color = _TrailColor.rgb * _Intensity * input.color.rgb * intensity;
                return half4(color, alpha);
            }
            ENDHLSL
        }
    }
}
