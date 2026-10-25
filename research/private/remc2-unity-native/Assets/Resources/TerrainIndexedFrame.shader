// Owner indices and PALN are external private-cache inputs. The native CPU
// renderer has already completed source sky, mode 5, and mode 26 indexed
// compositing before this shader runs.
Shader "MagicCarpet/TerrainIndexedFrame"
{
    Properties
    {
        _Indices ("Exact 320x200 indexed sky and terrain", 2D) = "black" {}
        _Coverage ("Exact terrain write coverage diagnostic", 2D) = "black" {}
        _Palette ("Ordered PALN lookup", 2D) = "white" {}
        _OutputIndices ("Output palette indices", Float) = 0
    }
    SubShader
    {
        Tags { "Queue"="Background" "RenderType"="Opaque" }
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
            sampler2D _Coverage;
            sampler2D _Palette;
            float _OutputIndices;

            fixed4 frag(v2f_img input) : SV_Target
            {
                fixed indexSample = tex2D(_Indices, input.uv).r;
                float index = floor(indexSample * 255.0 + 0.5);
                if (_OutputIndices > 0.5) return fixed4(index / 255.0, 0.0, 0.0, 1.0);
                fixed4 color = tex2D(
                    _Palette, float2((index + 0.5) / 256.0, 0.5));
                // Coverage deliberately does not control alpha. The exact
                // indexed buffer is sky-seeded, and mode 26 water has already
                // blended against those sky pixels on the CPU.
                color.a = 1.0;
                return color;
            }
            ENDCG
        }

    }
    FallBack Off
}
