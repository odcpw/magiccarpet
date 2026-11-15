Shader "MagicCarpet/SourceObjectiveTextbox"
{
    Properties
    {
        _Foreground ("Foreground indices and mask", 2D) = "black" {}
        _SceneIndices ("Current scene indices", 2D) = "black" {}
        _BlendRow ("Blend lookup row", 2D) = "black" {}
        _Palette ("Current palette", 2D) = "black" {}
        _PriorOverlay ("Indexed prior overlay", 2D) = "black" {}
        _PriorRect ("Indexed prior rectangle", Vector) = (0, 0, 0, 0)
        _PriorEnabled ("Use indexed prior overlay", Float) = 0
        _RenderPrior ("Render indexed prior overlay", Float) = 0
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
            sampler2D _Foreground, _SceneIndices, _BlendRow, _Palette, _PriorOverlay;
            float4 _Raster, _BlendRect, _PriorRect;
            float _SceneTopDown, _PriorEnabled, _RenderPrior;
            fixed4 frag(v2f_img input) : SV_Target
            {
                float2 local = float2(floor(input.uv.x * _Raster.z),
                    floor((1.0 - input.uv.y) * _Raster.w));
                float2 position = _Raster.xy + local;
                float sceneY = (position.y + 0.5) / 480.0;
                float2 sceneUv = float2((position.x + 0.5) / 640.0,
                    _SceneTopDown > 0.5 ? sceneY : 1.0 - sceneY);
                float index = floor(tex2D(_SceneIndices, sceneUv).r * 255.0 + 0.5);
                float priorCoverage = 0.0;
                if (_PriorEnabled > 0.5 && position.x >= _PriorRect.x && position.y >= _PriorRect.y &&
                    position.x < _PriorRect.x + _PriorRect.z && position.y < _PriorRect.y + _PriorRect.w)
                {
                    float2 priorLocal = position - _PriorRect.xy;
                    float2 priorUv = float2((priorLocal.x + 0.5) / _PriorRect.z,
                        1.0 - (priorLocal.y + 0.5) / _PriorRect.w);
                    fixed4 prior = tex2D(_PriorOverlay, priorUv);
                    if (prior.a > 0.5) index = floor(prior.r * 255.0 + 0.5);
                    if (prior.g > 0.5)
                    {
                        index = floor(tex2D(_BlendRow, float2((index + 0.5) / 256.0, 0.5)).r * 255.0 + 0.5);
                        index = floor(tex2D(_BlendRow, float2((index + 0.5) / 256.0, 0.5)).r * 255.0 + 0.5);
                    }
                    priorCoverage = max(prior.a, prior.g);
                }
                if (_RenderPrior > 0.5)
                {
                    if (priorCoverage < 0.5) return fixed4(0, 0, 0, 0);
                    return fixed4(tex2D(_Palette, float2((index + 0.5) / 256.0, 0.5)).rgb, 1.0);
                }
                fixed4 foreground = tex2D(_Foreground, input.uv);
                if (foreground.a > 0.5)
                    return fixed4(tex2D(_Palette, float2((floor(foreground.r * 255.0 + 0.5) + 0.5) / 256.0, 0.5)).rgb, 1.0);
                if (local.x < _BlendRect.x || local.y < _BlendRect.y ||
                    local.x >= _BlendRect.x + _BlendRect.z || local.y >= _BlendRect.y + _BlendRect.w)
                    return fixed4(0, 0, 0, 0);
                float blended = floor(tex2D(_BlendRow, float2((index + 0.5) / 256.0, 0.5)).r * 255.0 + 0.5);
                return fixed4(tex2D(_Palette, float2((blended + 0.5) / 256.0, 0.5)).rgb, 1.0);
            }
            ENDCG
        }
    }
}
