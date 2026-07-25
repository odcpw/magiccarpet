// Qualification-only bridge from source pnt5 fixed point to TABLESN.
// Owner lookup bytes remain external cache inputs.
Shader "MagicCarpet/TerrainPnt5Golden"
{
    Properties
    {
        _ShadeLut ("TABLESN lookup", 2D) = "white" {}
        _GoldenSourceIndex ("Source palette index", Float) = 0
        _GoldenPnt5Rows ("pnt5 divided by 65536", Float) = 0
    }
    SubShader
    {
        Tags { "RenderType"="Opaque" }
        Pass
        {
            ZTest Always Cull Off ZWrite Off
            CGPROGRAM
            #pragma vertex vert_img
            #pragma fragment frag
            #include "UnityCG.cginc"

            sampler2D _ShadeLut;
            float _GoldenSourceIndex;
            float _GoldenPnt5Rows;

            fixed4 frag(v2f_img input) : SV_Target
            {
                float whole = floor(_GoldenPnt5Rows);
                float brightness = whole - floor(whole / 256.0) * 256.0;
                fixed shaded = tex2D(
                    _ShadeLut,
                    float2(
                        (_GoldenSourceIndex + 0.5) / 256.0,
                        (brightness + 0.5) / 256.0)).r;
                return fixed4(shaded, shaded, shaded, 1.0);
            }
            ENDCG
        }
    }
    FallBack Off
}
