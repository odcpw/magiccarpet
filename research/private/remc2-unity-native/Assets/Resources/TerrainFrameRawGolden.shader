// Qualification-only shader for exact R8 upload/readback. No owner bytes are
// embedded here.
Shader "MagicCarpet/TerrainFrameRawGolden"
{
    Properties
    {
        _Indices ("R8 plane", 2D) = "black" {}
    }
    SubShader
    {
        Cull Off
        ZWrite Off
        ZTest Always

        Pass
        {
            CGPROGRAM
            #pragma vertex vert_img
            #pragma fragment frag
            #include "UnityCG.cginc"

            sampler2D _Indices;

            fixed4 frag(v2f_img input) : SV_Target
            {
                fixed value = tex2D(_Indices, input.uv).r;
                return fixed4(value, value, value, 1.0);
            }
            ENDCG
        }
    }
    FallBack Off
}
