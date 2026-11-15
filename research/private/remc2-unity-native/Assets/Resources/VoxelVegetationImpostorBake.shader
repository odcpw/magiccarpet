Shader "Hidden/MagicCarpet/VoxelVegetationImpostorBake"
{
    Properties { _CaptureData("Capture normal and depth", Float) = 0 }
    SubShader
    {
        Tags { "RenderPipeline"="UniversalPipeline" "RenderType"="Opaque" }
        Pass
        {
            ZWrite On Cull Back
            HLSLPROGRAM
            #pragma vertex Vert
            #pragma fragment Frag
            #include "Packages/com.unity.render-pipelines.universal/ShaderLibrary/Core.hlsl"
            float _CaptureData, _BakeDiameter;
            float4 _BakeCenter, _BakeDirection;
            struct A { float4 p:POSITION; float3 n:NORMAL; half4 c:COLOR; };
            struct V { float4 p:SV_POSITION; float3 position:TEXCOORD0; float3 normal:TEXCOORD1; half4 color:COLOR; };
            V Vert(A a)
            {
                V o; o.p=TransformObjectToHClip(a.p.xyz); o.position=a.p.xyz; o.normal=a.n; o.color=a.c; return o;
            }
            half4 Frag(V i):SV_Target
            {
                if (_CaptureData < .5) return half4(i.color.rgb, 1);
                return half4(normalize(i.normal)*.5+.5, dot(i.position-_BakeCenter.xyz,_BakeDirection.xyz)/_BakeDiameter+.5);
            }
            ENDHLSL
        }
    }
}
