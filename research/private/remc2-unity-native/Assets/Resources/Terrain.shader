// Kept in Resources so runtime construction cannot be stripped from players.
// Owner indices and PALN remain external cache inputs; this shader contains no
// original image or palette bytes.
Shader "MagicCarpet/Terrain"
{
    Properties
    {
        _TileArray ("Indexed BL32N tile array", 2DArray) = "" {}
        _ShadeLut ("TABLESN static shade lookup", 2D) = "white" {}
        _Palette ("Ordered PALN lookup", 2D) = "white" {}
        _Pnt5Rows ("Exact source pnt5 divided by 65536", 2D) = "black" {}
        _SurfaceDisplacement ("Exact source water displacement", 2D) = "black" {}
        _GridCellValid ("Exact source submitted cells", 2D) = "black" {}
        _SourceUnitsPerUnityUnit ("Source units per Unity unit", Float) = 64
    }
    SubShader
    {
        Tags { "RenderType"="Opaque" "Queue"="Geometry" }
        LOD 100

        Pass
        {
            CGPROGRAM
            #pragma vertex vert
            #pragma fragment frag
            #pragma target 3.5
            #pragma require 2darray
            #include "UnityCG.cginc"

            UNITY_DECLARE_TEX2DARRAY(_TileArray);
            sampler2D _ShadeLut;
            sampler2D _Palette;
            sampler2D _Pnt5Rows;
            sampler2D _SurfaceDisplacement;
            sampler2D _GridCellValid;
            float _SourceUnitsPerUnityUnit;

            struct appdata
            {
                float4 vertex : POSITION;
                float2 uv : TEXCOORD0;
                float2 selector : TEXCOORD1;
                float2 shade : TEXCOORD2;
                float2 mapVertex : TEXCOORD3;
                float2 mapCell : TEXCOORD4;
            };

            struct v2f
            {
                float4 position : SV_POSITION;
                float2 uv : TEXCOORD0;
                nointerpolation float selector : TEXCOORD1;
                float pnt5Rows : TEXCOORD2;
                nointerpolation float cellValid : TEXCOORD3;
            };

            v2f vert(appdata input)
            {
                v2f output;
                float4 displaced = input.vertex;
                displaced.y += tex2Dlod(
                    _SurfaceDisplacement,
                    float4(input.mapVertex, 0.0, 0.0)).r /
                    _SourceUnitsPerUnityUnit;
                output.position = UnityObjectToClipPos(displaced);
                output.uv = input.uv;
                output.selector = input.selector.x;
                output.pnt5Rows = tex2Dlod(
                    _Pnt5Rows, float4(input.mapVertex, 0.0, 0.0)).r;
                output.cellValid = tex2Dlod(
                    _GridCellValid, float4(input.mapCell, 0.0, 0.0)).r;
                return output;
            }

            fixed4 frag(v2f input) : SV_Target
            {
                clip(input.cellValid - 0.5);
                fixed indexSample =
                    UNITY_SAMPLE_TEX2DARRAY(
                        _TileArray, float3(input.uv, input.selector)).r;
                float sourceIndex = floor(indexSample * 255.0 + 0.5);
                // Vertex formation is exact. Unity's perspective interpolation
                // remains explicit deferred work until source mode 5 lands.
                float brightness = fmod(floor(input.pnt5Rows), 256.0);
                if (brightness < 0.0)
                    brightness += 256.0;
                fixed shadedSample = tex2D(
                    _ShadeLut,
                    float2(
                        (sourceIndex + 0.5) / 256.0,
                        (brightness + 0.5) / 256.0)).r;
                float index = floor(shadedSample * 255.0 + 0.5);
                fixed4 color = tex2D(
                    _Palette, float2((index + 0.5) / 256.0, 0.5));
                color.a = 1.0;
                return color;
            }
            ENDCG
        }

    }
    FallBack Off
}
