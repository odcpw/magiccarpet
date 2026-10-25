// DrawWorld_411A0 LABEL_33. The retained LUT contains the game's exact
// current-high/prior-low index mapping; this shader contains no owner bytes.
Shader "MagicCarpet/CampaignIndexedHistory"
{
    Properties
    {
        _MainTex ("Current composed world indices", 2D) = "black" {}
        _SceneHistory ("Previous composed world indices", 2D) = "black" {}
        _BlendLut ("Exact source indexed blend table", 2D) = "black" {}
        _BlendHistory ("Apply source mode2 history", Float) = 0
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
            #pragma fragment compose
            #include "UnityCG.cginc"
            sampler2D _MainTex;
            sampler2D _SceneHistory;
            sampler2D _BlendLut;
            float _BlendHistory;

            fixed4 compose(v2f_img input) : SV_Target
            {
                float current = floor(tex2D(_MainTex, input.uv).r * 255.0 + 0.5);
                if (_BlendHistory > 0.5)
                {
                    float prior = floor(tex2D(_SceneHistory, input.uv).r * 255.0 + 0.5);
                    current = floor(tex2D(_BlendLut, float2((prior + 0.5) / 256.0,
                        (current + 0.5) / 256.0)).r * 255.0 + 0.5);
                }
                return fixed4(current / 255.0, 0.0, 0.0, 1.0);
            }
            ENDCG
        }
    }
    FallBack Off
}
