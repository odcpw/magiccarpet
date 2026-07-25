// Kept in Resources so runtime construction cannot be stripped from players.
Shader "MagicCarpet/Surface"
{
    Properties
    {
        _Color ("Color", Color) = (1,1,1,1)
        _Glossiness ("Smoothness", Range(0,1)) = 0.2
        _Metallic ("Metallic", Range(0,1)) = 0.0
        _EmissionColor ("Emission", Color) = (0,0,0,0)
    }
    SubShader
    {
        Tags { "RenderType"="Opaque" }
        LOD 200

        CGPROGRAM
        #pragma surface surf Standard fullforwardshadows
        #pragma target 3.0

        fixed4 _Color;
        half _Glossiness;
        half _Metallic;
        fixed4 _EmissionColor;

        struct Input
        {
            float3 worldPos;
        };

        void surf(Input input, inout SurfaceOutputStandard output)
        {
            output.Albedo = _Color.rgb;
            output.Smoothness = _Glossiness;
            output.Metallic = _Metallic;
            output.Emission = _EmissionColor.rgb;
            output.Alpha = _Color.a;
        }
        ENDCG
    }
    FallBack "Diffuse"
}
