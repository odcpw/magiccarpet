// Diagnostic world-space depth for current Unity objects. This is not part of
// the exact 320x200 indexed terrain result and makes no exact occlusion claim.
Shader "MagicCarpet/TerrainDepthOnly"
{
    Properties
    {
        _SurfaceDisplacement ("Exact source water displacement", 2D) = "black" {}
        _GridCellValid ("Exact submitted terrain cells", 2D) = "black" {}
        _SourceUnitsPerUnityUnit ("Source units per Unity unit", Float) = 64
    }
    SubShader
    {
        Tags { "Queue"="Geometry-10" "RenderType"="Opaque" }

        Pass
        {
            Cull Back
            ZWrite On
            ZTest LEqual
            ColorMask 0

            CGPROGRAM
            #pragma vertex vert
            #pragma fragment frag
            #include "UnityCG.cginc"

            sampler2D _SurfaceDisplacement;
            sampler2D _GridCellValid;
            float _SourceUnitsPerUnityUnit;

            struct appdata
            {
                float4 vertex : POSITION;
                float2 mapVertex : TEXCOORD3;
                float2 mapCell : TEXCOORD4;
            };

            struct v2f
            {
                float4 position : SV_POSITION;
                nointerpolation float cellValid : TEXCOORD0;
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
                output.cellValid = tex2Dlod(
                    _GridCellValid, float4(input.mapCell, 0.0, 0.0)).r;
                return output;
            }

            fixed4 frag(v2f input) : SV_Target
            {
                clip(input.cellValid - 0.5);
                return 0;
            }
            ENDCG
        }
    }
    FallBack Off
}
