Shader "MagicCarpet/ClassicHd/IndexedUnlit"
{
    Properties
    {
        _IndexTex ("Indexed R8 Surface", 2D) = "black" {}
        _PaletteTex ("Resolved Owner Palette", 2D) = "white" {}
    }

    SubShader
    {
        Tags { "RenderType" = "Opaque" "Queue" = "Overlay" "RenderPipeline" = "UniversalPipeline" }

        Pass
        {
            ZTest Always
            ZWrite Off
            Cull Off

            HLSLPROGRAM
            #pragma vertex Vert
            #pragma fragment Frag

            #include "Packages/com.unity.render-pipelines.universal/ShaderLibrary/Core.hlsl"

            TEXTURE2D(_IndexTex);
            SAMPLER(sampler_IndexTex);
            TEXTURE2D(_PaletteTex);
            SAMPLER(sampler_PaletteTex);

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
                half index = SAMPLE_TEXTURE2D(_IndexTex, sampler_IndexTex, input.uv).r;
                half paletteU = (index * 255.0h + 0.5h) / 256.0h;
                return SAMPLE_TEXTURE2D(_PaletteTex, sampler_PaletteTex, float2(paletteU, 0.5h));
            }
            ENDHLSL
        }
    }
}
