Shader "Hidden/MagicCarpet/TerrainGolden"
{
    Properties
    {
        _TileArray ("Indexed BL32N tile array", 2DArray) = "" {}
        _ShadeLut ("TABLESN static shade lookup", 2D) = "white" {}
        _GoldenSelector ("Golden selector", Float) = 0
        _GoldenU ("Golden U", Float) = 0
        _GoldenV ("Golden V", Float) = 0
        _GoldenBrightness ("Golden brightness", Float) = 0
    }
    SubShader
    {
        Pass
        {
            CGPROGRAM
            #pragma vertex vert_img
            #pragma fragment golden
            #pragma target 3.5
            #pragma require 2darray
            #include "UnityCG.cginc"

            UNITY_DECLARE_TEX2DARRAY(_TileArray);
            sampler2D _ShadeLut;
            float _GoldenSelector;
            float _GoldenU;
            float _GoldenV;
            float _GoldenBrightness;

            fixed4 golden(v2f_img input) : SV_Target
            {
                fixed sourceSample = UNITY_SAMPLE_TEX2DARRAY(
                    _TileArray,
                    float3(_GoldenU, _GoldenV, _GoldenSelector)).r;
                float sourceIndex = floor(sourceSample * 255.0 + 0.5);
                fixed shadedSample = tex2D(
                    _ShadeLut,
                    float2(
                        (sourceIndex + 0.5) / 256.0,
                        (_GoldenBrightness + 0.5) / 256.0)).r;
                return fixed4(shadedSample, shadedSample, shadedSample, 1.0);
            }
            ENDCG
        }
    }
    FallBack Off
}
