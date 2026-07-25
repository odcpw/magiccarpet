// Owner indices and PALN remain external cache inputs. This shader contains no
// original image or palette bytes.
Shader "MagicCarpet/EntitySprite"
{
    Properties
    {
        _Indices ("Indexed TMAP frame", 2D) = "black" {}
        _Palette ("Ordered PALN lookup", 2D) = "white" {}
    }
    SubShader
    {
        Tags { "Queue"="AlphaTest" "RenderType"="TransparentCutout" }
        Cull Off
        ZWrite On
        ZTest LEqual

        Pass
        {
            CGPROGRAM
            #pragma vertex vert
            #pragma fragment frag
            #pragma multi_compile_fog
            #include "UnityCG.cginc"

            sampler2D _Indices;
            sampler2D _Palette;

            struct appdata
            {
                float4 vertex : POSITION;
                float2 uv : TEXCOORD0;
            };

            struct v2f
            {
                float4 position : SV_POSITION;
                float2 uv : TEXCOORD0;
                UNITY_FOG_COORDS(1)
            };

            v2f vert(appdata input)
            {
                v2f output;
                output.position = UnityObjectToClipPos(input.vertex);
                output.uv = input.uv;
                UNITY_TRANSFER_FOG(output, output.position);
                return output;
            }

            fixed4 frag(v2f input) : SV_Target
            {
                fixed sampleValue = tex2D(_Indices, input.uv).r;
                float index = floor(sampleValue * 255.0 + 0.5);
                clip(index - 0.5);
                fixed4 color = tex2D(
                    _Palette, float2((index + 0.5) / 256.0, 0.5));
                color.a = 1.0;
                UNITY_APPLY_FOG(input.fogCoord, color);
                return color;
            }
            ENDCG
        }
    }
    FallBack Off
}
