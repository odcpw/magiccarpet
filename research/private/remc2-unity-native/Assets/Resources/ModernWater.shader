Shader "MagicCarpet/ModernWater"
{
    Properties
    {
        _DeepColor ("Deep water", Color) = (0.012, 0.055, 0.075, 1)
        _ShallowColor ("Shallow water", Color) = (0.08, 0.24, 0.21, 1)
        _FoamColor ("Foam", Color) = (0.66, 0.76, 0.68, 1)
        _WaveStrength ("Wave strength", Float) = 0.12
        _ReflectionTexture ("Planar reflection", 2D) = "black" {}
        _UsePlanarReflection ("Use planar reflection", Float) = 0
        _ReflectionDistortion ("Reflection ripple distortion", Float) = 0.025
    }
    SubShader
    {
        Tags { "RenderPipeline"="UniversalPipeline" "RenderType"="Transparent" "Queue"="Transparent" }
        Pass
        {
            Name "WaterForward"
            Tags { "LightMode"="UniversalForward" }
            Blend SrcAlpha OneMinusSrcAlpha
            ZWrite Off
            Cull Back
            HLSLPROGRAM
            #pragma target 3.5
            #pragma vertex Vert
            #pragma fragment Frag
            #pragma multi_compile _ _CLUSTER_LIGHT_LOOP
            #pragma multi_compile _ _MAIN_LIGHT_SHADOWS _MAIN_LIGHT_SHADOWS_CASCADE _MAIN_LIGHT_SHADOWS_SCREEN
            #pragma multi_compile_fragment _ _SHADOWS_SOFT _SHADOWS_SOFT_LOW _SHADOWS_SOFT_MEDIUM _SHADOWS_SOFT_HIGH
            #pragma multi_compile_fog
            #define _SURFACE_TYPE_TRANSPARENT 1
            #include "Packages/com.unity.render-pipelines.universal/ShaderLibrary/Core.hlsl"
            #include "Packages/com.unity.render-pipelines.universal/ShaderLibrary/Lighting.hlsl"
            #include "Packages/com.unity.render-pipelines.universal/ShaderLibrary/DeclareDepthTexture.hlsl"
            TEXTURE2D(_ReflectionTexture); SAMPLER(sampler_ReflectionTexture);
            CBUFFER_START(UnityPerMaterial)
                half4 _DeepColor, _ShallowColor, _FoamColor;
                float _WaveStrength, _UsePlanarReflection, _ReflectionDistortion;
                float4x4 _ReflectionVP;
            CBUFFER_END
            // Global pass guard: the reflection camera must never sample its own target.
            float _ModernReflectionPass;
            struct Attributes { float4 positionOS : POSITION; float3 normalOS : NORMAL; half4 color : COLOR; };
            struct Varyings { float4 positionCS : SV_POSITION; float3 positionWS : TEXCOORD0; half3 normalWS : TEXCOORD1; half4 color : COLOR; half fog : TEXCOORD2; };
            Varyings Vert(Attributes v)
            {
                Varyings o; float3 world = TransformObjectToWorld(v.positionOS.xyz);
                // Very small displacement preserves shore contact and avoids cracks between cells.
                world.y += (1-saturate(v.color.a)) * _WaveStrength * (sin(dot(world.xz,float2(0.22,0.15))+_Time.y*0.65) + sin(dot(world.xz,float2(-0.11,0.3))-_Time.y*0.47))*0.5;
                o.positionWS = world; o.positionCS = TransformWorldToHClip(world);
                o.normalWS = TransformObjectToWorldNormal(v.normalOS); o.color = v.color;
                o.fog = ComputeFogFactor(o.positionCS.z); return o;
            }
            // Integer corners are stable across neighboring lattice cells.
            float Hash(int2 p)
            {
                uint2 bits=asuint(p);
                uint h=bits.x*0x8da6b343u ^ bits.y*0xd8163841u ^ 0xcb1ab31fu;
                h^=h>>16; h*=0x7feb352du; h^=h>>15; h*=0x846ca68bu; h^=h>>16;
                return (h & 0x00ffffffu)*(1.0/16777216.0);
            }
            // Value and analytic gradient of smooth value noise, in texture space.
            float3 NoiseGradient(float2 p)
            {
                int2 i=(int2)floor(p); float2 f=frac(p);
                float2 u=f*f*(3-2*f), du=6*f*(1-f);
                float a=Hash(i), b=Hash(i+int2(1,0));
                float c=Hash(i+int2(0,1)), d=Hash(i+int2(1,1));
                float value=lerp(lerp(a,b,u.x),lerp(c,d,u.x),u.y);
                float dx=lerp(b-a,d-c,u.y)*du.x;
                float dy=lerp(c-a,d-b,u.x)*du.y;
                return float3(value,dx,dy);
            }
            half4 Frag(Varyings i) : SV_Target
            {
                clip(0.5-_ModernReflectionPass);
                float2 p=i.positionWS.xz; float t=_Time.y;
                float footprint=length(fwidth(p));
                float3 broad=NoiseGradient(p*0.18+float2(t*0.045,-t*0.026));
                float3 mid=NoiseGradient(p*0.57+float2(-t*0.09,t*0.055)+13.4);
                float3 fine=NoiseGradient(p*1.25+float2(t*0.13,t*0.07)-8.1);
                float3 micro=NoiseGradient(p*3.4+float2(-t*0.16,t*0.12)+23.6);
                float midFade=1-smoothstep(0.5,2.0,footprint);
                float fineFade=1-smoothstep(0.20,0.9,footprint);
                float microFade=1-smoothstep(0.08,0.35,footprint);
                float2 ripple=broad.yz*0.08 + mid.yz*(0.11*midFade)
                    + fine.yz*(0.10*fineFade) + micro.yz*(0.055*microFade);
                half3 n=normalize(i.normalWS + half3(-ripple.x,0,-ripple.y));
                half3 view=GetWorldSpaceNormalizeViewDir(i.positionWS);
                float2 screenUV=GetNormalizedScreenSpaceUV(i.positionCS);
                float sceneDepth=LinearEyeDepth(SampleSceneDepth(screenUV),_ZBufferParams);
                float waterDepth=-TransformWorldToView(i.positionWS).z;
                float depth=max(0,sceneDepth-waterDepth);
                float shore=saturate(i.color.a);
                float shallow=max(1-saturate(depth*0.18),shore*0.45);
                float fresnel=0.02+0.98*pow(1-saturate(dot(n,view)),5);
                half3 reflectionDir=reflect(-view,n);
                half3 reflected=lerp(half3(0.23,0.27,0.31),half3(0.025,0.055,0.11),
                    pow(saturate(reflectionDir.y),0.48));
                if (_UsePlanarReflection > 0.5)
                {
                    // ReflectionVP is the render-to-texture GPU projection
                    // multiplied by the reflected camera view matrix.
                    float4 reflectionClip=mul(_ReflectionVP,float4(p.x,0,p.y,1));
                    #if UNITY_UV_STARTS_AT_TOP
                        reflectionClip.y=-reflectionClip.y;
                    #endif
                    float2 reflectionUV=reflectionClip.xy/max(reflectionClip.w,0.0001)*0.5+0.5;
                    reflectionUV+=ripple*_ReflectionDistortion;
                    float border=min(min(reflectionUV.x,reflectionUV.y),
                        min(1-reflectionUV.x,1-reflectionUV.y));
                    float valid=step(0.0001,reflectionClip.w)*smoothstep(0.0,0.035,border);
                    half3 sceneReflection=SAMPLE_TEXTURE2D(_ReflectionTexture,sampler_ReflectionTexture,
                        saturate(reflectionUV)).rgb;
                    reflected=lerp(reflected,sceneReflection,valid);
                }
                half3 color=lerp(_DeepColor.rgb,_ShallowColor.rgb,shallow);
                color=lerp(color,reflected,saturate(0.08+fresnel*0.92));
                Light light=GetMainLight(TransformWorldToShadowCoord(i.positionWS));
                half3 h=normalize(light.direction+view);
                float spec=pow(saturate(dot(n,h)),160)*0.8 + pow(saturate(dot(n,h)),40)*0.12;
                color += light.color*light.shadowAttenuation*spec;
                float foamNoise=lerp(0.5,mid.x,midFade)*0.5+lerp(0.5,fine.x,fineFade)*0.35
                    +lerp(0.5,micro.x,microFade)*0.15;
                float foamDepth=(1-smoothstep(0.05,0.9,depth))*0.4;
                float foam=max(foamDepth,pow(shore,3.0))*smoothstep(0.48,0.72,foamNoise);
                color=lerp(color,_FoamColor.rgb,foam*0.25);
                color=MixFog(color,i.fog);
                return half4(color,lerp(0.82,0.96,fresnel));
            }
            ENDHLSL
        }
    }
    FallBack Off
}
