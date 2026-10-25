// Owner indices and PALN remain external cache inputs. This shader contains no
// original image or palette bytes.
Shader "MagicCarpet/EntitySprite"
{
    Properties
    {
        _Indices ("Indexed TMAP frame", 2D) = "black" {}
        _Palette ("Ordered PALN lookup", 2D) = "white" {}
        _CaveOrders ("Cave terrain source-cell order", 2D) = "black" {}
        _CaveCellOrder ("Sprite source-cell order", Float) = 0
        _CaveOcclusionEnabled ("Use Cave source-cell occlusion", Float) = 0
        _OutputIndices ("Output palette indices", Float) = 0
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
            sampler2D _CaveOrders;
            float _CaveCellOrder;
            float _CaveOcclusionEnabled;
            float _OutputIndices;

            struct appdata
            {
                float4 vertex : POSITION;
                float2 uv : TEXCOORD0;
            };

            struct v2f
            {
                float4 position : SV_POSITION;
                float2 uv : TEXCOORD0;
                float4 screenPosition : TEXCOORD1;
                UNITY_FOG_COORDS(2)
            };

            v2f vert(appdata input)
            {
                v2f output;
                output.position = UnityObjectToClipPos(input.vertex);
                output.uv = input.uv;
                output.screenPosition = ComputeScreenPos(output.position);
                UNITY_TRANSFER_FOG(output, output.position);
                return output;
            }

            fixed4 frag(v2f input) : SV_Target
            {
                fixed sampleValue = tex2D(_Indices, input.uv).r;
                float index = floor(sampleValue * 255.0 + 0.5);
                clip(index - 0.5);
                if (_CaveOcclusionEnabled > 0.5)
                {
                    // Source draws a sprite bucket after that cell's floor
                    // and ceiling. A later terrain write hides only the
                    // covered sprite fragments; equality remains visible.
                    clip(_CaveCellOrder - 0.5);
                    float2 screenUv = input.screenPosition.xy / input.screenPosition.w;
                    float2 orderBytes = tex2D(_CaveOrders, float2(screenUv.x, 1.0 - screenUv.y)).rg;
                    float terrainOrder = floor(orderBytes.r * 255.0 + 0.5)
                        + floor(orderBytes.g * 255.0 + 0.5) * 256.0;
                    clip(_CaveCellOrder - terrainOrder);
                }
                if (_OutputIndices > 0.5) return fixed4(index / 255.0, 0.0, 0.0, 1.0);
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
