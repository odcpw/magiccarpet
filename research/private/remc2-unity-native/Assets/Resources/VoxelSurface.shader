Shader "MagicCarpet/VoxelSurface"
{
 Properties { _BaseColor("Tint", Color)=(1,1,1,1) _Alpha("Opacity", Range(0,1))=1 _Emission("Emission", Float)=0 }
 SubShader {
 Tags { "RenderType"="Opaque" "RenderPipeline"="UniversalPipeline" "Queue"="Geometry" }
 HLSLINCLUDE
 #include "Packages/com.unity.render-pipelines.universal/ShaderLibrary/Core.hlsl"
 CBUFFER_START(UnityPerMaterial)
 half4 _BaseColor;
 half _Alpha;
 half _Emission;
 CBUFFER_END
 void Fade(float2 p) { clip(_Alpha - frac(52.9829189 * frac(dot(p, float2(.06711056,.00583715)))) - .00001); }
 ENDHLSL
 Pass {
 Name "ForwardLit" Tags { "LightMode"="UniversalForward" }
 HLSLPROGRAM
 #pragma vertex Vert
 #pragma fragment Frag
 #pragma multi_compile _ _MAIN_LIGHT_SHADOWS _MAIN_LIGHT_SHADOWS_CASCADE _MAIN_LIGHT_SHADOWS_SCREEN
 #pragma multi_compile_fragment _ _SHADOWS_SOFT
 #pragma multi_compile_fog
 #include "Packages/com.unity.render-pipelines.universal/ShaderLibrary/Lighting.hlsl"
 struct A { float4 p:POSITION; float3 n:NORMAL; half4 c:COLOR; };
 struct V { float4 p:SV_POSITION; float3 w:TEXCOORD0; float3 n:TEXCOORD1; half4 c:COLOR; half fog:TEXCOORD2; };
 V Vert(A a) { V o; o.w=TransformObjectToWorld(a.p.xyz); o.p=TransformWorldToHClip(o.w); o.n=TransformObjectToWorldNormal(a.n); o.c=a.c*_BaseColor; o.fog=ComputeFogFactor(o.p.z); return o; }
 half4 Frag(V i):SV_Target { Fade(i.p.xy); float3 n=normalize(i.n); Light l=GetMainLight(TransformWorldToShadowCoord(i.w)); half3 ambient=SampleSH(n); half3 color=i.c.rgb*(ambient + l.color*saturate(dot(n,l.direction))*l.shadowAttenuation + _Emission); return half4(MixFog(color,i.fog),1); }
 ENDHLSL
 }
 Pass {
 Name "ShadowCaster" Tags { "LightMode"="ShadowCaster" } ZWrite On ZTest LEqual ColorMask 0
 HLSLPROGRAM
 #pragma vertex Vert
 #pragma fragment Frag
 #include "Packages/com.unity.render-pipelines.universal/ShaderLibrary/Shadows.hlsl"
 float3 _LightDirection;
 struct A { float4 p:POSITION; float3 n:NORMAL; };
 struct V { float4 p:SV_POSITION; };
 V Vert(A a) { V o; float3 w=TransformObjectToWorld(a.p.xyz); o.p=TransformWorldToHClip(ApplyShadowBias(w,TransformObjectToWorldNormal(a.n),_LightDirection));
 #if UNITY_REVERSED_Z
 o.p.z=min(o.p.z,UNITY_NEAR_CLIP_VALUE);
 #else
 o.p.z=max(o.p.z,UNITY_NEAR_CLIP_VALUE);
 #endif
 return o; }
 half4 Frag(V i):SV_Target { Fade(i.p.xy); return 0; }
 ENDHLSL
 }
 Pass {
 Name "DepthOnly" Tags { "LightMode"="DepthOnly" } ZWrite On ColorMask R
 HLSLPROGRAM
 #pragma vertex Vert
 #pragma fragment Frag
 struct A { float4 p:POSITION; }; struct V { float4 p:SV_POSITION; };
 V Vert(A a) { V o; o.p=TransformObjectToHClip(a.p.xyz); return o; }
 half4 Frag(V i):SV_Target { Fade(i.p.xy); return 0; }
 ENDHLSL
 }
 }
}
