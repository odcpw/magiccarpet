Shader "MagicCarpet/SourceObjectiveTextbox"
{
    Properties
    {
        _Foreground ("Foreground indices and mask", 2D) = "black" {}
        _SceneIndices ("Current scene indices", 2D) = "black" {}
        _BlendRow ("Blend lookup row", 2D) = "black" {}
        _Palette ("Current palette", 2D) = "black" {}
        _Raster ("Source raster geometry", Vector) = (0, 0, 0, 0)
        _BlendRect ("Blend geometry", Vector) = (0, 0, 0, 0)
        _SceneTopDown ("Top-down source", Float) = 0
    }
    SubShader
    {
        Cull Off ZWrite Off ZTest Always
        Pass
        {
            CGPROGRAM
            #pragma vertex vert_img
            #pragma fragment frag
            #include "UnityCG.cginc"
            sampler2D _Foreground, _SceneIndices, _BlendRow, _Palette;
            float4 _Raster, _BlendRect;
            float _SceneTopDown;
            fixed4 frag(v2f_img input) : SV_Target
            {
                float2 local = float2(floor(input.uv.x * _Raster.z),
                    floor((1.0 - input.uv.y) * _Raster.w));
                fixed4 foreground = tex2D(_Foreground, input.uv);
                if (foreground.a > 0.5)
                    return fixed4(tex2D(_Palette, float2((floor(foreground.r * 255.0 + 0.5) + 0.5) / 256.0, 0.5)).rgb, 1.0);
                if (local.x < _BlendRect.x || local.y < _BlendRect.y ||
                    local.x >= _BlendRect.x + _BlendRect.z || local.y >= _BlendRect.y + _BlendRect.w)
                    return fixed4(0, 0, 0, 0);
                float sceneY = (_Raster.y + local.y + 0.5) / 480.0;
                float2 sceneUv = float2((_Raster.x + local.x + 0.5) / 640.0,
                    _SceneTopDown > 0.5 ? sceneY : 1.0 - sceneY);
                float index = floor(tex2D(_SceneIndices, sceneUv).r * 255.0 + 0.5);
                float blended = floor(tex2D(_BlendRow, float2((index + 0.5) / 256.0, 0.5)).r * 255.0 + 0.5);
                return fixed4(tex2D(_Palette, float2((blended + 0.5) / 256.0, 0.5)).rgb, 1.0);
            }
            ENDCG
        }
    }
}
