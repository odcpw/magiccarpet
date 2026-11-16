// Indexed world -> WEB -> source minimap terrain -> source-ordered marks.
Shader "MagicCarpet/CampaignIndexedMinimap"
{
    Properties
    {
        _MainTex ("Current indexed scene", 2D) = "black" {}
        _MapFunctions ("Per-pixel source index functions", 2D) = "black" {}
        _MapOutside ("Opaque labels outside minimap", 2D) = "black" {}
        _Web ("Source WEB index and coverage", 2D) = "black" {}
        _Palette ("Current palette", 2D) = "black" {}
        _SceneTopDown ("CPU scene row order", Float) = 0
        _MapVisible ("Circular minimap visible", Float) = 1
        _WebVisible ("Source WEB visible", Float) = 0
    }
    SubShader
    {
        Cull Off ZWrite Off ZTest Always
        CGINCLUDE
        #include "UnityCG.cginc"
        sampler2D _MainTex, _MapFunctions, _MapOutside, _Web, _Palette;
        float _SceneTopDown, _MapVisible, _WebVisible;
        fixed4 compose(v2f_img input) : SV_Target
        {
            float2 top = float2(input.uv.x, 1.0 - input.uv.y);
            float value = floor(tex2D(_MainTex, _SceneTopDown > .5 ? top : input.uv).r * 255.0 + .5);
            if (_WebVisible > .5)
            {
                fixed4 web = tex2D(_Web, top);
                if (web.a > .5) value = floor(web.r * 255.0 + .5);
            }
            if (_MapVisible > .5)
            {
                int2 pixel = min(int2(319, 199), int2(floor(top * float2(320, 200))));
                if (pixel.x < 64 && pixel.y < 64)
                    value = floor(tex2D(_MapFunctions, float2((value + .5) / 256.0,
                        (pixel.y * 64 + pixel.x + .5) / 4096.0)).r * 255.0 + .5);
                else
                {
                    fixed4 outside = tex2D(_MapOutside, top);
                    if (outside.a > .5) value = floor(outside.r * 255.0 + .5);
                }
            }
            return fixed4(value / 255.0, 0, 0, 1);
        }
        fixed4 display(v2f_img input) : SV_Target
        {
            float index = floor(tex2D(_MainTex, input.uv).r * 255.0 + .5);
            return fixed4(tex2D(_Palette, float2((index + .5) / 256.0, .5)).rgb, 1);
        }
        ENDCG
        Pass
        {
            CGPROGRAM
            #pragma target 3.0
            #pragma vertex vert_img
            #pragma fragment compose
            ENDCG
        }
        Pass
        {
            CGPROGRAM
            #pragma vertex vert_img
            #pragma fragment display
            ENDCG
        }
    }
    FallBack Off
}
