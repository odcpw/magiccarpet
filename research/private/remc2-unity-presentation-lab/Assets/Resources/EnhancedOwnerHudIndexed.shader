Shader "MagicCarpet/Enhanced/OwnerHudIndexed"
{
    Properties
    {
        _IndexTex ("Owner Indexed HUD", 2D) = "black" {}
        _ProvenanceTex ("Owner HUD Provenance", 2D) = "black" {}
        _PaletteTex ("Owner Palette RGB8", 2D) = "white" {}
    }

    SubShader
    {
        Tags
        {
            "RenderType" = "Transparent"
            "Queue" = "Overlay"
            "RenderPipeline" = "UniversalPipeline"
        }

        Pass
        {
            ZTest Always
            ZWrite Off
            Cull Off
            Blend SrcAlpha OneMinusSrcAlpha, One Zero

            HLSLPROGRAM
            #pragma vertex Vert
            #pragma fragment Frag

            #include "Packages/com.unity.render-pipelines.universal/ShaderLibrary/Core.hlsl"

            TEXTURE2D(_IndexTex);
            SAMPLER(sampler_IndexTex);
            TEXTURE2D(_ProvenanceTex);
            SAMPLER(sampler_ProvenanceTex);
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
                uint provenance = (uint)round(
                    SAMPLE_TEXTURE2D(_ProvenanceTex, sampler_ProvenanceTex, input.uv).r * 255.0h);
                if ((provenance & 8u) == 0u)
                {
                    return half4(0.0h, 0.0h, 0.0h, 0.0h);
                }

                half index = SAMPLE_TEXTURE2D(_IndexTex, sampler_IndexTex, input.uv).r;
                half paletteU = (index * 255.0h + 0.5h) / 256.0h;
                half4 colour = SAMPLE_TEXTURE2D(_PaletteTex, sampler_PaletteTex, float2(paletteU, 0.5h));
                return half4(colour.rgb, 1.0h);
            }
            ENDHLSL
        }
    }
}
