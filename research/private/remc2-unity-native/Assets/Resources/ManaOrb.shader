Shader "MagicCarpet/ManaOrb"
{
    Properties { _BaseColor("Source tint", Color) = (1,1,1,1) }
    SubShader
    {
        Tags { "RenderType"="Opaque" "RenderPipeline"="UniversalPipeline" "Queue"="Geometry" }
        Pass
        {
            Name "Forward" Tags { "LightMode"="UniversalForward" }
            HLSLPROGRAM
            #pragma vertex Vert
            #pragma fragment Frag
            #pragma multi_compile_fog
            #include "Packages/com.unity.render-pipelines.universal/ShaderLibrary/Core.hlsl"
            CBUFFER_START(UnityPerMaterial)
            half4 _BaseColor;
            CBUFFER_END
            struct Attributes { float4 positionOS : POSITION; float3 normalOS : NORMAL; };
            struct Varyings { float4 positionCS : SV_POSITION; float3 positionWS : TEXCOORD0; half3 normalWS : TEXCOORD1; half fog : TEXCOORD2; };
            Varyings Vert(Attributes input)
            {
                Varyings output;
                output.positionWS = TransformObjectToWorld(input.positionOS.xyz);
                output.positionCS = TransformWorldToHClip(output.positionWS);
                output.normalWS = TransformObjectToWorldNormal(input.normalOS);
                output.fog = ComputeFogFactor(output.positionCS.z);
                return output;
            }
            half4 Frag(Varyings input) : SV_Target
            {
                half3 n = normalize(input.normalWS);
                half3 view = normalize(GetWorldSpaceViewDir(input.positionWS));
                half facing = saturate(dot(n, view));
                half rim = pow(1.0h - facing, 2.5h);
                half core = pow(facing, 2.0h);
                half3 highlightDirection = normalize(half3(-0.38h, 0.8h, -0.46h) + view);
                half glint = pow(saturate(dot(n, highlightDirection)), 48.0h);
                half upper = saturate(n.y * 0.5h + 0.5h);
                // Emissive shading remains readable in darkness while the
                // source tint continues to identify the native owner.
                half3 color = _BaseColor.rgb * (0.8h + 0.45h * upper + 0.5h * core + 0.65h * rim);
                color += glint * (0.45h + 0.55h * _BaseColor.rgb);
                return half4(MixFog(color, input.fog), 1);
            }
            ENDHLSL
        }
    }
}
