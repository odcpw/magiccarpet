Shader "MagicCarpet/Enhanced/ActorContactShadow"
{
    Properties
    {
        _ShadowColor ("Shadow Color", Color) = (0.015, 0.008, 0.02, 1)
        _Opacity ("Opacity", Range(0, 1)) = 0.28
        _Softness ("Softness", Range(0.05, 1)) = 0.72
    }

    SubShader
    {
        Tags
        {
            "Queue" = "Transparent+5"
            "RenderType" = "Transparent"
            "RenderPipeline" = "UniversalPipeline"
        }

        Blend SrcAlpha OneMinusSrcAlpha
        ZWrite Off
        ZTest LEqual
        Cull Off

        Pass
        {
            Name "OwnerActorContactShadow"
            Tags { "LightMode" = "UniversalForward" }

            HLSLPROGRAM
            #pragma vertex Vert
            #pragma fragment Frag

            #include "Packages/com.unity.render-pipelines.universal/ShaderLibrary/Core.hlsl"

            CBUFFER_START(UnityPerMaterial)
                half4 _ShadowColor;
                half _Opacity;
                half _Softness;
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
                float2 centered = input.uv * 2.0 - 1.0;
                float radius = length(centered);
                float softEdge = lerp(0.58, 0.84, saturate(_Softness));
                half radial = 1.0h - smoothstep(0.04, softEdge, radius);
                radial *= 1.0h - smoothstep(0.78, 1.0, radius);
                half alpha = radial * saturate(_Opacity) * _ShadowColor.a;
                return half4(_ShadowColor.rgb, alpha);
            }
            ENDHLSL
        }
    }
    FallBack Off
}
