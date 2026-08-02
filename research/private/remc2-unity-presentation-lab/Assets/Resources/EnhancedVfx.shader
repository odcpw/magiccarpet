Shader "MagicCarpet/Enhanced/Vfx"
{
    Properties
    {
        [HDR] _GlowColor ("Glow Color", Color) = (1, 0.2, 0.05, 1)
        _GlowIntensity ("Glow Intensity", Range(0, 8)) = 2
        _Opacity ("Opacity", Range(0, 1)) = 0.85
        _PulseSpeed ("Pulse Speed", Float) = 2.5
        _RimStrength ("Rim Strength", Range(0, 4)) = 1.2
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
            Name "VfxGlow"
            Tags { "LightMode" = "UniversalForward" }

            HLSLPROGRAM
            #pragma vertex Vert
            #pragma fragment Frag

            #include "Packages/com.unity.render-pipelines.universal/ShaderLibrary/Core.hlsl"

            CBUFFER_START(UnityPerMaterial)
                half4 _GlowColor;
                half _GlowIntensity;
                half _Opacity;
                half _PulseSpeed;
                half _RimStrength;
            CBUFFER_END

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
            };

            Varyings Vert(Attributes input)
            {
                Varyings output;
                VertexPositionInputs positionInputs = GetVertexPositionInputs(input.positionOS.xyz);
                VertexNormalInputs normalInputs = GetVertexNormalInputs(input.normalOS);
                output.positionHCS = positionInputs.positionCS;
                output.positionWS = positionInputs.positionWS;
                output.normalWS = normalInputs.normalWS;
                return output;
            }

            half4 Frag(Varyings input) : SV_Target
            {
                half3 normalWS = NormalizeNormalPerPixel(input.normalWS);
                half3 viewDirectionWS = GetWorldSpaceNormalizeViewDir(input.positionWS);
                half rim = pow(saturate(1.0h - dot(normalWS, viewDirectionWS)), 2.0h);
                half pulse = 0.88h + 0.12h * sin(_Time.y * _PulseSpeed);
                half intensity = _GlowIntensity * pulse * (0.72h + rim * _RimStrength);
                return half4(_GlowColor.rgb * intensity, saturate(_Opacity * (0.75h + rim * 0.25h)));
            }
            ENDHLSL
        }
    }
}
