Shader "MagicCarpet/ModernVegetationImpostor"
{
    Properties
    {
        _BaseMap ("Multiview color and coverage", 2D) = "white" {}
        _ImpostorData ("Object normal and view depth", 2D) = "black" {}
        _ImpostorCenter ("Source mesh center", Vector) = (0,0,0,0)
        _ImpostorDiameter ("Source bounding sphere diameter", Float) = 1
        _WindStrength ("Wind strength", Float) = 0.16
        _WindSpeed ("Wind speed", Float) = 1.0
        _WindMinY ("Wind base height", Float) = 0
        _WindInvHeight ("Inverse wind height", Float) = 1
        _Smoothness ("Smoothness", Range(0,1)) = 0.15
        _Cutoff ("Alpha cutoff", Range(0,1)) = 0.4
        _LodThreshold ("Signed detail coverage", Float) = 0
    }
    SubShader
    {
        Tags { "RenderPipeline"="UniversalPipeline" "RenderType"="Opaque" "Queue"="Geometry" }
        Cull Off
        HLSLINCLUDE
        #include "Packages/com.unity.render-pipelines.universal/ShaderLibrary/Core.hlsl"
        #include "Packages/com.unity.render-pipelines.universal/ShaderLibrary/Lighting.hlsl"
        TEXTURE2D(_BaseMap); SAMPLER(sampler_BaseMap);
        TEXTURE2D(_ImpostorData); SAMPLER(sampler_ImpostorData);
        CBUFFER_START(UnityPerMaterial)
            float4 _ImpostorCenter;
            float _ImpostorDiameter, _WindStrength, _WindSpeed, _WindMinY;
            float _WindInvHeight, _Smoothness, _Cutoff;
        CBUFFER_END
        UNITY_INSTANCING_BUFFER_START(VegetationLod)
            UNITY_DEFINE_INSTANCED_PROP(float, _LodThreshold)
        UNITY_INSTANCING_BUFFER_END(VegetationLod)
        float _ModernVegetationDebug;

        void ApplyLodFade(float2 pixel)
        {
            float threshold=UNITY_ACCESS_INSTANCED_PROP(VegetationLod, _LodThreshold);
            if (threshold <= 0.0) return;
            float noise=frac(52.9829189*frac(dot(floor(pixel),float2(0.06711056,0.00583715))));
            clip(noise >= threshold ? 1.0 : -1.0);
        }

        float4 _ModernWind, _ModernWindOffset;
        float3 WindPosition(float3 world, float3 local)
        {
            float weight=saturate((local.y-_WindMinY)*_WindInvHeight);
            float2 p=(world.xz-_ModernWindOffset.xy)*0.006135923;
            float sway=sin(dot(p,float2(23,15)))+0.35*sin(dot(p,float2(41,27))+world.y*0.21);
            float2 direction=_ModernWind.xy/max(0.1,length(_ModernWind.xy));
            world.xz+=direction*sway*_WindStrength*weight*min(2,_ModernWind.z);
            return world;
        }

        // The imported FBX basis is Unity=(-BlenderX, BlenderZ, -BlenderY).
        // This frame matches the camera basis used for every offline capture.
        void ViewFrame(float3 directionWS, out float3 directionOS, out float2 frame,
            out float3 rightOS, out float3 upOS)
        {
            float3 d=TransformWorldToObjectDir(directionWS, false);
            d/=max(length(d),1e-6);
            float horizontal=length(d.xz);
            float azimuth=horizontal > 1e-6 ? atan2(d.z,d.x) : 0.0;
            float ca=cos(azimuth), sa=sin(azimuth);
            rightOS=float3(-sa,0,ca);
            upOS=float3(-d.y*ca,horizontal,-d.y*sa);
            directionOS=d;
            frame=float2(frac(azimuth/(2.0*PI)+1.0)*8.0,
                (asin(clamp(d.y,-1.0,1.0))/PI+0.5)*4.0);
        }

        float3 BillboardPosition(float2 corner, float3 rightOS, float3 upOS)
        {
            // 128px tiles contain a 112px capture and eight guard pixels per side.
            return _ImpostorCenter.xyz+(rightOS*corner.x+upOS*corner.y)*(_ImpostorDiameter*(128.0/112.0));
        }

        float2 AtlasUV(float2 uv, float azimuth, float elevation)
        {
            return (float2(fmod(azimuth+8.0,8.0),clamp(elevation,0.0,4.0))+uv)*float2(1.0/8.0,1.0/5.0);
        }

        void AddFrame(float2 uv, float azimuth, float elevation, float weight,
            inout float coverage, inout float3 color, inout float4 data)
        {
            float2 atlas=AtlasUV(uv,azimuth,elevation);
            float4 c=SAMPLE_TEXTURE2D(_BaseMap,sampler_BaseMap,atlas);
            float4 d=SAMPLE_TEXTURE2D(_ImpostorData,sampler_ImpostorData,atlas);
            float contribution=weight*c.a;
            coverage+=contribution;
            color+=contribution*c.rgb;
            data+=contribution*d;
        }

        void SampleImpostor(float2 uv, float2 frame, out half4 color, out float4 data)
        {
            float2 cell=floor(frame), fraction=frac(frame);
            float coverage=0.0; float3 rgb=0.0; data=0.0;
            AddFrame(uv,cell.x,cell.y,(1.0-fraction.x)*(1.0-fraction.y),coverage,rgb,data);
            AddFrame(uv,cell.x+1.0,cell.y,fraction.x*(1.0-fraction.y),coverage,rgb,data);
            AddFrame(uv,cell.x,cell.y+1.0,(1.0-fraction.x)*fraction.y,coverage,rgb,data);
            AddFrame(uv,cell.x+1.0,cell.y+1.0,fraction.x*fraction.y,coverage,rgb,data);
            clip(coverage-_Cutoff);
            float inverseCoverage=rcp(max(coverage,1e-5));
            color=half4(rgb*inverseCoverage,coverage);
            data*=inverseCoverage;
        }

        float3 SurfacePosition(float3 planeWS, float3 directionOS, float depth)
        {
            float3 offsetOS=directionOS*((depth-0.5)*_ImpostorDiameter);
            return planeWS+TransformObjectToWorldDir(offsetOS,false);
        }

        float DeviceDepth(float4 clipPosition)
        {
            float depth=clipPosition.z/clipPosition.w;
            #if defined(SHADER_API_GLCORE) || defined(SHADER_API_GLES3)
                depth=depth*0.5+0.5;
            #endif
            return depth;
        }
        ENDHLSL

        Pass
        {
            Name "ForwardLit"
            Tags { "LightMode"="UniversalForward" }
            HLSLPROGRAM
            #pragma target 3.5
            #pragma vertex Vert
            #pragma fragment Frag
            #pragma multi_compile_instancing
            #pragma multi_compile _ _CLUSTER_LIGHT_LOOP
            #pragma multi_compile _ _MAIN_LIGHT_SHADOWS _MAIN_LIGHT_SHADOWS_CASCADE _MAIN_LIGHT_SHADOWS_SCREEN
            #pragma multi_compile_fragment _ _SHADOWS_SOFT _SHADOWS_SOFT_LOW _SHADOWS_SOFT_MEDIUM _SHADOWS_SOFT_HIGH
            #pragma multi_compile_fog
            struct Attributes { float4 positionOS:POSITION; float2 uv:TEXCOORD0; UNITY_VERTEX_INPUT_INSTANCE_ID };
            struct Varyings { float4 positionCS:SV_POSITION; float3 planeWS:TEXCOORD0; float3 directionOS:TEXCOORD1; float2 frame:TEXCOORD2; float2 uv:TEXCOORD3; UNITY_VERTEX_INPUT_INSTANCE_ID };
            struct Output { half4 color:SV_Target; float depth:SV_Depth; };
            Varyings Vert(Attributes v)
            {
                UNITY_SETUP_INSTANCE_ID(v); Varyings o; UNITY_TRANSFER_INSTANCE_ID(v,o);
                float3 centerWS=TransformObjectToWorld(_ImpostorCenter.xyz);
                float3 rightOS,upOS;
                ViewFrame(GetWorldSpaceViewDir(centerWS),o.directionOS,o.frame,rightOS,upOS);
                float3 local=BillboardPosition(v.positionOS.xy,rightOS,upOS);
                o.planeWS=WindPosition(TransformObjectToWorld(local),local);
                o.positionCS=TransformWorldToHClip(o.planeWS); o.uv=v.uv; return o;
            }
            Output Frag(Varyings i)
            {
                UNITY_SETUP_INSTANCE_ID(i); ApplyLodFade(i.positionCS.xy);
                half4 color; float4 data; SampleImpostor(i.uv,i.frame,color,data);
                float3 positionWS=SurfacePosition(i.planeWS,i.directionOS,data.a);
                half3 n=normalize(TransformObjectToWorldNormal(normalize(data.rgb*2.0-1.0)));
                InputData input=(InputData)0; input.positionWS=positionWS; input.normalWS=n;
                input.viewDirectionWS=GetWorldSpaceNormalizeViewDir(positionWS);
                input.shadowCoord=TransformWorldToShadowCoord(positionWS);
                input.bakedGI=SampleSH(n); input.normalizedScreenSpaceUV=GetNormalizedScreenSpaceUV(i.positionCS);
                input.shadowMask=half4(1,1,1,1);
                SurfaceData surface=(SurfaceData)0; surface.albedo=color.rgb; surface.metallic=0;
                surface.specular=half3(0.04,0.04,0.04); surface.smoothness=_Smoothness;
                surface.occlusion=1; surface.alpha=1; surface.normalTS=half3(0,0,1);
                float4 clipPosition=TransformWorldToHClip(positionWS);
                Output o; o.color=UniversalFragmentPBR(input,surface);
                o.color.rgb=MixFog(o.color.rgb,ComputeFogFactor(clipPosition.z));
                if (_ModernVegetationDebug > 0.5 && _ModernVegetationDebug < 1.5) o.color=half4(saturate(input.bakedGI),1);
                if (_ModernVegetationDebug > 1.5 && _ModernVegetationDebug < 2.5) o.color=half4(0.5,0.5,0.5,1);
                if (_ModernVegetationDebug > 2.5 && _ModernVegetationDebug < 3.5)
                {
                    Light direct=GetMainLight(input.shadowCoord);
                    half intensity=saturate(dot(n,direct.direction))*direct.distanceAttenuation*direct.shadowAttenuation;
                    o.color=half4(intensity,intensity,intensity,1);
                }
                o.depth=DeviceDepth(clipPosition); return o;
            }
            ENDHLSL
        }

        Pass
        {
            Name "ShadowCaster"
            Tags { "LightMode"="ShadowCaster" }
            ZWrite On ZTest LEqual ColorMask 0
            HLSLPROGRAM
            #pragma target 3.5
            #pragma vertex ShadowVert
            #pragma fragment ShadowFrag
            #pragma multi_compile_instancing
            #pragma multi_compile_vertex _ _CASTING_PUNCTUAL_LIGHT_SHADOW
            float3 _LightDirection;
            float3 _LightPosition;
            struct Attributes { float4 positionOS:POSITION; float2 uv:TEXCOORD0; UNITY_VERTEX_INPUT_INSTANCE_ID };
            struct Varyings { float4 positionCS:SV_POSITION; float3 planeWS:TEXCOORD0; float3 directionOS:TEXCOORD1; float2 frame:TEXCOORD2; float2 uv:TEXCOORD3; UNITY_VERTEX_INPUT_INSTANCE_ID };
            Varyings ShadowVert(Attributes v)
            {
                UNITY_SETUP_INSTANCE_ID(v); Varyings o; UNITY_TRANSFER_INSTANCE_ID(v,o);
                float3 centerWS=TransformObjectToWorld(_ImpostorCenter.xyz);
                #if _CASTING_PUNCTUAL_LIGHT_SHADOW
                    float3 direction=_LightPosition-centerWS;
                #else
                    float3 direction=_LightDirection;
                #endif
                float3 rightOS,upOS;
                ViewFrame(direction,o.directionOS,o.frame,rightOS,upOS);
                float3 local=BillboardPosition(v.positionOS.xy,rightOS,upOS);
                o.planeWS=WindPosition(TransformObjectToWorld(local),local);
                o.positionCS=ApplyShadowClamping(TransformWorldToHClip(o.planeWS)); o.uv=v.uv; return o;
            }
            float ShadowFrag(Varyings i):SV_Depth
            {
                UNITY_SETUP_INSTANCE_ID(i); ApplyLodFade(i.positionCS.xy);
                half4 color; float4 data; SampleImpostor(i.uv,i.frame,color,data);
                float3 positionWS=SurfacePosition(i.planeWS,i.directionOS,data.a);
                float3 normalWS=normalize(TransformObjectToWorldNormal(normalize(data.rgb*2.0-1.0)));
                #if _CASTING_PUNCTUAL_LIGHT_SHADOW
                    float3 lightDirection=normalize(_LightPosition-positionWS);
                #else
                    float3 lightDirection=_LightDirection;
                #endif
                float4 clipPosition=ApplyShadowClamping(TransformWorldToHClip(ApplyShadowBias(positionWS,normalWS,lightDirection)));
                return DeviceDepth(clipPosition);
            }
            ENDHLSL
        }

        Pass
        {
            Name "DepthOnly"
            Tags { "LightMode"="DepthOnly" }
            ZWrite On ColorMask R
            HLSLPROGRAM
            #pragma target 3.5
            #pragma vertex DepthVert
            #pragma fragment DepthFrag
            #pragma multi_compile_instancing
            struct Attributes { float4 positionOS:POSITION; float2 uv:TEXCOORD0; UNITY_VERTEX_INPUT_INSTANCE_ID };
            struct Varyings { float4 positionCS:SV_POSITION; float3 planeWS:TEXCOORD0; float3 directionOS:TEXCOORD1; float2 frame:TEXCOORD2; float2 uv:TEXCOORD3; UNITY_VERTEX_INPUT_INSTANCE_ID };
            Varyings DepthVert(Attributes v)
            {
                UNITY_SETUP_INSTANCE_ID(v); Varyings o; UNITY_TRANSFER_INSTANCE_ID(v,o);
                float3 centerWS=TransformObjectToWorld(_ImpostorCenter.xyz);
                float3 rightOS,upOS;
                ViewFrame(GetWorldSpaceViewDir(centerWS),o.directionOS,o.frame,rightOS,upOS);
                float3 local=BillboardPosition(v.positionOS.xy,rightOS,upOS);
                o.planeWS=WindPosition(TransformObjectToWorld(local),local);
                o.positionCS=TransformWorldToHClip(o.planeWS); o.uv=v.uv; return o;
            }
            float DepthFrag(Varyings i):SV_Depth
            {
                UNITY_SETUP_INSTANCE_ID(i); ApplyLodFade(i.positionCS.xy);
                half4 color; float4 data; SampleImpostor(i.uv,i.frame,color,data);
                float3 positionWS=SurfacePosition(i.planeWS,i.directionOS,data.a);
                return DeviceDepth(TransformWorldToHClip(positionWS));
            }
            ENDHLSL
        }
    }
    FallBack Off
}
