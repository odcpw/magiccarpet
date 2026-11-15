Shader "MagicCarpet/ModernFireball"
{
    Properties
    {
        _BaseColor ("Flame colour", Color) = (1, .32, .035, 1)
        _CoreColor ("Core colour", Color) = (1, .7, .12, 1)
        _Emission ("Radiance", Float) = 1.4
        _Trail ("Trailing flame", Range(0,1)) = 1
    }
    SubShader
    {
        Tags { "RenderPipeline"="UniversalPipeline" "RenderType"="Opaque" "Queue"="Geometry" }
        Pass
        {
            Name "Fireball"
            Tags { "LightMode"="UniversalForward" }
            Cull Back
            ZWrite On
            HLSLPROGRAM
            #pragma vertex Vert
            #pragma fragment Frag
            #pragma multi_compile_fog
            #include "Packages/com.unity.render-pipelines.universal/ShaderLibrary/Core.hlsl"
            CBUFFER_START(UnityPerMaterial)
                half4 _BaseColor;
                half4 _CoreColor;
                half _Emission;
                half _Trail;
            CBUFFER_END
            struct Attributes { float4 positionOS : POSITION; float3 normalOS : NORMAL; };
            struct Varyings
            {
                float4 positionCS : SV_POSITION;
                float3 positionWS : TEXCOORD0;
                float3 local : TEXCOORD1;
                float3 normalWS : TEXCOORD2;
                half fog : TEXCOORD3;
            };
            Varyings Vert(Attributes input)
            {
                Varyings o;
                float3 p = input.positionOS.xyz;
                float flicker = sin(dot(p, float3(17, 11, 23)) - _Time.y * 13);
                p *= 1 + .055 * flicker;
                // A short trailing flame follows the observed direction of travel.
                // It is visual geometry; no collider, integration or bounce exists.
                p.z -= max(0, -p.z) * .65 * _Trail;
                o.local = input.positionOS.xyz;
                o.positionWS = TransformObjectToWorld(p);
                o.positionCS = TransformWorldToHClip(o.positionWS);
                o.normalWS = TransformObjectToWorldNormal(input.normalOS);
                o.fog = ComputeFogFactor(o.positionCS.z);
                return o;
            }
            half4 Frag(Varyings i) : SV_Target
            {
                float t = _Time.y;
                float ribbons = sin(dot(i.local, float3(8, 11, 9)) - t * 9)
                    + .45 * sin(dot(i.local, float3(-11, 17, 6)) + t * 7);
                float facing = saturate(dot(normalize(i.normalWS), normalize(GetWorldSpaceViewDir(i.positionWS))));
                float heat = saturate(.27 + .49 * facing + .17 * ribbons);
                half3 colour = lerp(_BaseColor.rgb * .45, _CoreColor.rgb, heat);
                colour = lerp(colour, lerp(_CoreColor.rgb, half3(1,1,1), .65), smoothstep(.88, 1, heat));
                colour *= _Emission;
                return half4(MixFog(colour, i.fog), 1);
            }
            ENDHLSL
        }
    }
    FallBack Off
}
