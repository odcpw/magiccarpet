Shader "MagicCarpet/ModernWater"
{
    Properties
    {
        _DeepColor ("Deep water", Color) = (0.012, 0.055, 0.075, 1)
        _ShallowColor ("Shallow water", Color) = (0.08, 0.24, 0.21, 1)
        _FoamColor ("Foam", Color) = (0.66, 0.76, 0.68, 1)
        _WaveStrength ("Conservative displacement bound", Float) = 3
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
            #pragma target 4.5
            #pragma vertex Vert
            #pragma fragment Frag
            #pragma multi_compile _ _MAIN_LIGHT_SHADOWS _MAIN_LIGHT_SHADOWS_CASCADE _MAIN_LIGHT_SHADOWS_SCREEN
            #pragma multi_compile_fragment _ _SHADOWS_SOFT _SHADOWS_SOFT_LOW _SHADOWS_SOFT_MEDIUM _SHADOWS_SOFT_HIGH
            #pragma multi_compile_fog
            #define _SURFACE_TYPE_TRANSPARENT 1
            #include "Packages/com.unity.render-pipelines.universal/ShaderLibrary/Core.hlsl"
            #include "Packages/com.unity.render-pipelines.universal/ShaderLibrary/Lighting.hlsl"
            #include "Packages/com.unity.render-pipelines.universal/ShaderLibrary/DeclareDepthTexture.hlsl"
            TEXTURE2D(_ReflectionTexture); SAMPLER(sampler_ReflectionTexture);
            TEXTURE2D(_WaterDisplacement); SAMPLER(sampler_WaterDisplacement);
            TEXTURE2D(_WaterNormalFoam); SAMPLER(sampler_WaterNormalFoam);
            TEXTURE2D(_WaterDetailNormal); SAMPLER(sampler_WaterDetailNormal);
            TEXTURE2D(_WaterCoast); SAMPLER(sampler_WaterCoast);
            TEXTURE2D(_WaterLocal); SAMPLER(sampler_WaterLocal);
            TEXTURE2D(_WaterPattern); SAMPLER(sampler_WaterPattern);
            CBUFFER_START(UnityPerMaterial)
                half4 _DeepColor, _ShallowColor, _FoamColor;
                float _WaveStrength, _UsePlanarReflection, _ReflectionDistortion;
                float4x4 _ReflectionVP;
            CBUFFER_END
            float _ModernReflectionPass;
            float4 _WaterLocalRect, _ModernWind, _ModernWindOffset;
            float _WaterTime;
            struct Attributes { float4 positionOS : POSITION; float3 normalOS : NORMAL; half4 color : COLOR; };
            struct Varyings {
                float4 positionCS : SV_POSITION;
                float3 positionWS : TEXCOORD0;
                float2 rest : TEXCOORD1;
                half fog : TEXCOORD2;
            };
            float2 LocalUV(float2 p) { return (p-_WaterLocalRect.xy)*_WaterLocalRect.w; }
            float LocalMask(float2 uv) {
                float edge=min(min(uv.x,uv.y),min(1-uv.x,1-uv.y));
                return smoothstep(0,0.06,edge);
            }
            float Envelope(float d) { return smoothstep(0,12,d); }
            float3 Surface(float2 p)
            {
                float4 coast=SAMPLE_TEXTURE2D_LOD(_WaterCoast,sampler_WaterCoast,p/1024,0);
                float3 d=SAMPLE_TEXTURE2D_LOD(_WaterDisplacement,sampler_WaterDisplacement,p/256+0.5/256,0).rgb*Envelope(coast.r);
                float2 uv=LocalUV(p);
                d.y+=SAMPLE_TEXTURE2D_LOD(_WaterLocal,sampler_WaterLocal,saturate(uv),0).r*LocalMask(uv)*smoothstep(0,2,coast.r);
                float exposure=saturate(-dot(normalize(_ModernWind.xy+0.001),coast.ba));
                d.y+=sin(max(0,coast.r)*.8+_WaterTime*1.25)*exp(-max(0,coast.r)*.18)
                    *smoothstep(0,2,coast.r)*.15*saturate(_ModernWind.z)*(.2+.8*exposure);
                return float3(p.x,0,p.y)+d;
            }
            Varyings Vert(Attributes v)
            {
                Varyings o;
                float3 world=TransformObjectToWorld(v.positionOS.xyz);
                o.rest=world.xz;
                // Both horizontal and vertical waves vanish at the source shoreline.
                // The short spectrum is shading-only on the current two-metre mesh.
                world+=Surface(world.xz)-float3(world.x,0,world.z);
                o.positionWS=world; o.positionCS=TransformWorldToHClip(world);
                o.fog=ComputeFogFactor(o.positionCS.z); return o;
            }
            half4 Frag(Varyings i) : SV_Target
            {
                clip(0.5-_ModernReflectionPass);
                float2 p=i.rest;
                float4 coast=SAMPLE_TEXTURE2D(_WaterCoast,sampler_WaterCoast,p/1024);
                float envelope=Envelope(coast.r);
                float2 detail=SAMPLE_TEXTURE2D(_WaterDetailNormal,sampler_WaterDetailNormal,p/32+0.5/256).rg;
                float footprint=max(length(ddx(p)),length(ddy(p)));
                float detailFade=1-smoothstep(1,6,footprint);
                // Differentiate the entire displaced surface, including horizontal chop
                // and both coastal/local envelopes. This preserves shore lighting.
                float epsilon=max(.5,min(2,footprint));
                float3 tx=Surface(p+float2(epsilon,0))-Surface(p-float2(epsilon,0));
                float3 tz=Surface(p+float2(0,epsilon))-Surface(p-float2(0,epsilon));
                float3 geometric=normalize(cross(tz,tx));
                float2 slope=-geometric.xz/max(.15,geometric.y)+detail*detailFade*envelope;
                float2 luv=LocalUV(p); float localMask=LocalMask(luv);
                float4 local=SAMPLE_TEXTURE2D(_WaterLocal,sampler_WaterLocal,saturate(luv));
                half3 n=normalize(half3(-slope.x,1,-slope.y));
                half3 view=GetWorldSpaceNormalizeViewDir(i.positionWS);
                float2 screenUV=GetNormalizedScreenSpaceUV(i.positionCS);
                float sceneDepth=LinearEyeDepth(SampleSceneDepth(screenUV),_ZBufferParams);
                float waterDepth=-TransformWorldToView(i.positionWS).z;
                float intersection=max(0,sceneDepth-waterDepth);
                float fresnel=0.02+0.98*pow(1-saturate(dot(n,view)),5);
                half3 reflectionDir=reflect(-view,n);
                half3 reflected=lerp(half3(0.23,0.27,0.31),half3(0.025,0.055,0.11),pow(saturate(reflectionDir.y),0.48));
                if (_UsePlanarReflection > 0.5)
                {
                    float4 reflectionClip=mul(_ReflectionVP,float4(p.x,0,p.y,1));
                    #if UNITY_UV_STARTS_AT_TOP
                        reflectionClip.y=-reflectionClip.y;
                    #endif
                    float2 reflectionUV=reflectionClip.xy/max(reflectionClip.w,0.0001)*0.5+0.5;
                    reflectionUV-=slope*_ReflectionDistortion;
                    float border=min(min(reflectionUV.x,reflectionUV.y),min(1-reflectionUV.x,1-reflectionUV.y));
                    float valid=step(0.0001,reflectionClip.w)*smoothstep(0,0.035,border);
                    reflected=lerp(reflected,SAMPLE_TEXTURE2D_LOD(_ReflectionTexture,sampler_ReflectionTexture,saturate(reflectionUV),1.1+2*(1-detailFade)).rgb,valid);
                }
                // Coast G is explicitly authored visual bed depth, never camera depth.
                half3 color=lerp(_ShallowColor.rgb,_DeepColor.rgb,1-exp(-coast.g*0.19));
                color=lerp(color,reflected,saturate(0.08+fresnel*0.92));
                Light light=GetMainLight(TransformWorldToShadowCoord(i.positionWS));
                half3 h=normalize(light.direction+view);
                float rough=lerp(100,45,1-detailFade);
                float spec=pow(saturate(dot(n,h)),rough)*0.65;
                color+=light.color*light.shadowAttenuation*spec;
                float2 drift=_ModernWindOffset.xy*0.015625;
                float pattern=SAMPLE_TEXTURE2D(_WaterPattern,sampler_WaterPattern,(p-drift)/16).r;
                float bubbles=SAMPLE_TEXTURE2D(_WaterPattern,sampler_WaterPattern,(p-drift*0.75)/4).g;
                float exposure=saturate(-dot(normalize(_ModernWind.xy+0.001),coast.ba));
                float lap=pow(saturate(sin(max(0,coast.r)*0.8+_WaterTime*1.25)),8);
                float shore=exp(-max(0,coast.r)*0.24)*lap*(0.25+0.75*exposure);
                float coverage=max(local.b*localMask,shore*0.6*(1-localMask));
                coverage=max(coverage,exp(-intersection*5)*0.22);
                // History is coverage, not an opaque decal. Break it into drifting
                // patches, with small cellular bubble walls inside those patches.
                float foam=smoothstep(.12,.7,coverage)*smoothstep(.25,.72,pattern*.65+bubbles*.35);
                color=lerp(color,_FoamColor.rgb*(0.6+0.4*light.shadowAttenuation),foam);
                color=MixFog(color,i.fog);
                return half4(color,lerp(0.94,1,max(fresnel,foam)));
            }
            ENDHLSL
        }
    }
    FallBack Off
}
