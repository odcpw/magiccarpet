Shader "MagicCarpet/ModernSteamVolume"
{
    Properties { _VolumeOpacity ("Volume opacity", Range(0, 1)) = 1 }
    SubShader
    {
        Tags { "RenderPipeline"="UniversalPipeline" "Queue"="Transparent+12" "RenderType"="Transparent" }
        Pass
        {
            Name "SteamVolume"
            Blend One OneMinusSrcAlpha
            ZWrite Off ZTest Always Cull Front
            HLSLPROGRAM
            #pragma target 4.5
            #pragma vertex Vert
            #pragma fragment Frag
            #include "Packages/com.unity.render-pipelines.universal/ShaderLibrary/Core.hlsl"
            #include "Packages/com.unity.render-pipelines.universal/ShaderLibrary/Lighting.hlsl"
            #include "Packages/com.unity.render-pipelines.universal/ShaderLibrary/DeclareDepthTexture.hlsl"
            #include "ModernLocalLighting.hlsl"
            #include "ModernSteamOptics.hlsl"
            TEXTURE3D(_Density); SAMPLER(sampler_Density);
            TEXTURE2D(_WaterCoast); SAMPLER(sampler_WaterCoast);
            TEXTURE2D(_WaterDisplacement); SAMPLER(sampler_WaterDisplacement);
            CBUFFER_START(UnityPerMaterial)
            float4 _Volume;
            float _Clock;
            float _VolumeOpacity;
            CBUFFER_END
            struct A { float4 positionOS:POSITION; };
            struct V { float4 positionCS:SV_POSITION; float3 world:TEXCOORD0; };
            V Vert(A i) { V o; o.world=TransformObjectToWorld(i.positionOS.xyz); o.positionCS=TransformWorldToHClip(o.world); return o; }
            float Density(float3 p)
            {
                float3 uv=(p-_Volume.xyz)/_Volume.w;
                if(any(uv<0)||any(uv>1)) return 0;
                float3 edge=min(uv,1-uv)*_Volume.w;
                float boundary=smoothstep(0,.8,min(edge.x,min(edge.y,edge.z)));
                return dot(SAMPLE_TEXTURE3D_LOD(_Density,sampler_Density,uv,0),float4(1,1,1,1))*boundary;
            }
            half4 Frag(V i):SV_Target
            {
                float3 ro=GetCameraPositionWS(), rd=normalize(i.world-ro);
                float3 inv=sign(rd+1e-8)/max(abs(rd),1e-6);
                float3 a=(_Volume.xyz-ro)*inv, b=(_Volume.xyz+_Volume.w-ro)*inv;
                float3 lo=min(a,b), hi=max(a,b);
                float near=max(0,max(lo.x,max(lo.y,lo.z))), far=min(hi.x,min(hi.y,hi.z));
                float raw=SampleSceneDepth(GetNormalizedScreenSpaceUV(i.positionCS));
                float eye=LinearEyeDepth(raw,_ZBufferParams);
                float3 viewRay=TransformWorldToViewDir(rd,false);
                far=min(far,eye/max(.0001,-viewRay.z));
                if(far<=near) return 0;
                Light sun=GetMainLight();
                float facing=dot(rd,sun.direction);
                float phase=(1-.3*.3)/pow(max(.05,1+.3*.3-2*.3*facing),1.5);
                float3 ambient=max(float3(.025,.035,.05),SampleSH(float3(0,1,0))*.55);
                float trans=1; float3 light=0;
                // The default 48³ field spans 12 m (0.25 m per voxel), so
                // oversampling it at 0.10 m costs much more inside a plume.
                float stepSize=max(.25,(far-near)/48);
                // Select at most two lights once per volume, not all eight at
                // every density step. Use the same bounds-based ranking for every
                // pixel: ray-dependent rankings can swap lights across an image
                // boundary and draw a false straight edge through the fog.
                int2 localIndex=int2(-1,-1);
                float2 localScore=0;
                [loop] for(int k=0; k<_ModernLightCount; ++k)
                {
                    float3 position=_ModernLightPosition[k].xyz;
                    float3 closest=clamp(position,_Volume.xyz,_Volume.xyz+_Volume.w);
                    float3 delta=position-closest;
                    float d2=dot(delta,delta), range=_ModernLightPosition[k].w;
                    float edge=saturate(1-d2/max(range*range,.001));
                    float score=edge*edge*_ModernLightColor[k].w/(1+d2*.16);
                    if(score>localScore.x) { localScore.y=localScore.x; localIndex.y=localIndex.x; localScore.x=score; localIndex.x=k; }
                    else if(score>localScore.y) { localScore.y=score; localIndex.y=k; }
                }
                // Fixed pixel dither, no history reconstruction or time-dependent sparkle.
                float jitter=frac(52.9829189*frac(dot(i.positionCS.xy,float2(.06711056,.00583715))));
                for(float t=near+stepSize*jitter; t<far && trans>.015; t+=stepSize)
                {
                    float3 p=ro+rd*t;
                    float3 samplePosition; float multiplier,detail;
                    ModernSteamOptics(p,_Clock,samplePosition,multiplier,detail);
                    // Reject only after warping: a coarse-density early-out cuts
                    // off lobes where the optical shape extends into empty voxels.
                    float density=Density(samplePosition)*multiplier
                        *ModernSteamBoundary(p,_Volume.xyz,_Volume.w);
                    if(density<.000005) continue;
                    float coast=SAMPLE_TEXTURE2D_LOD(_WaterCoast,sampler_WaterCoast,p.xz/1024,0).r;
                    float water=SAMPLE_TEXTURE2D_LOD(_WaterDisplacement,sampler_WaterDisplacement,p.xz/256,0).g*smoothstep(0,12,coast);
                    if(coast>0) density*=smoothstep(water-.04,water+.16,p.y);
                    float optical=density*150; // m^-1, 10um water droplets, Qext approximately 2.
                    float shadow=0;
                    [unroll] for(int s=1;s<=4;s++) shadow+=Density(p+sun.direction*(s*.45))*.45*150;
                    // The same advected sub-cell density detail also affects
                    // unresolved light paths: thinner pockets admit more light.
                    // This uses the two existing noise samples, not new texture
                    // reads or changes to the conserved simulation field.
                    shadow*=lerp(.65,1.35,detail);
                    float alpha=1-exp(-optical*min(stepSize,far-t));
                    // Single scattering alone makes a dense backlit water cloud
                    // collapse to an almost black core with a bright rim. A small
                    // isotropic key-light term approximates light redistributed by
                    // multiple scattering, while the directional term retains the
                    // measured density's soft self-shadow and phase response.
                    float multiple=.35*lerp(1.35,.65,detail);
                    float3 illumination=ambient+sun.color*(multiple+.16*phase*exp(-shadow));
                    // Isotropic local scattering is intentionally restrained. It
                    // changes illumination only; the simulated density and optical
                    // extinction remain the same with the torches on or off.
                    [unroll] for(int l=0; l<2; ++l)
                    {
                        int index=localIndex[l];
                        if(index<0) continue;
                        float3 delta=_ModernLightPosition[index].xyz-p;
                        float d2=dot(delta,delta), range=_ModernLightPosition[index].w;
                        float edge=saturate(1-d2/max(range*range,.001));
                        illumination+=_ModernLightColor[index].rgb*(.10*edge*edge/(1+d2*.16))*ModernEmitterMask(index,p);
                    }
                    light+=trans*alpha*illumination; trans*=1-alpha;
                }
                return half4(light,1-trans)*_VolumeOpacity;
            }
            ENDHLSL
        }
    }
}
