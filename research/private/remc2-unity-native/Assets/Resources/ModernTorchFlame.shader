Shader "MagicCarpet/ModernTorchFlame"
{
    Properties
    {
        _FlameSeed ("Stable fixture variation", Float) = 0
    }
    SubShader
    {
        Tags { "RenderPipeline"="UniversalPipeline" "Queue"="Transparent+20" "RenderType"="Transparent" }
        Pass
        {
            Name "TorchFlame"
            Tags { "LightMode"="UniversalForward" }
            Blend SrcAlpha OneMinusSrcAlpha
            ZWrite Off Cull Off
            HLSLPROGRAM
            #pragma target 3.5
            #pragma vertex Vert
            #pragma fragment Frag
            #pragma multi_compile_fog
            #include "Packages/com.unity.render-pipelines.universal/ShaderLibrary/Core.hlsl"
            #include "ModernSteamOptics.hlsl"
            CBUFFER_START(UnityPerMaterial)
                float _FlameSeed;
            CBUFFER_END
            float4 _ModernWind;
            // The four bounded persistent fields are published by their owner in
            // the same wrapped display frame as the steam volume renderer.
            TEXTURE3D(_ModernTorchSteam0); SAMPLER(sampler_ModernTorchSteam0);
            TEXTURE3D(_ModernTorchSteam1); SAMPLER(sampler_ModernTorchSteam1);
            TEXTURE3D(_ModernTorchSteam2); SAMPLER(sampler_ModernTorchSteam2);
            TEXTURE3D(_ModernTorchSteam3); SAMPLER(sampler_ModernTorchSteam3);
            float4 _ModernTorchSteamBounds[4];
            float4 _ModernTorchSteamClock;
            struct A { float4 positionOS:POSITION; float3 normalOS:NORMAL; float2 uv:TEXCOORD0; float4 color:COLOR; };
            struct V { float4 positionCS:SV_POSITION; float2 uv:TEXCOORD0; float3 world:TEXCOORD1; half3 normal:TEXCOORD2; half ember:TEXCOORD3; half fog:TEXCOORD4; };
            V Vert(A i)
            {
                V o;
                float3 world=TransformObjectToWorld(i.positionOS.xyz);
                float height=i.uv.y*(1-i.color.r);
                float t=_Time.y;
                float flutter=sin(t*4.7+_FlameSeed+height*5)*.065+sin(t*8.1+_FlameSeed*1.7-height*4)*.035;
                float2 wind=_ModernWind.xy*min(1,40/max(length(_ModernWind.xy),.001));
                world.xz+=wind*.018*height*height+float2(flutter,flutter*.6)*height*height;
                world.y+=sin(t*4.2+_FlameSeed)*.035*height*height;
                o.world=world; o.positionCS=TransformWorldToHClip(world); o.uv=i.uv;
                o.normal=TransformObjectToWorldNormal(i.normalOS); o.ember=i.color.r;
                o.fog=ComputeFogFactor(o.positionCS.z); return o;
            }
            float Hash(float2 p)
            {
                float3 v=frac(float3(p.xyx)*.1031);
                v+=dot(v,v.yzx+33.33);
                return frac((v.x+v.y)*v.z);
            }
            float Noise(float2 p)
            {
                float2 cell=floor(p), f=frac(p); f=f*f*(3-2*f);
                return lerp(lerp(Hash(cell),Hash(cell+float2(1,0)),f.x),
                    lerp(Hash(cell+float2(0,1)),Hash(cell+1),f.x),f.y);
            }
            float SteamDensity(int index, float3 uv)
            {
                if(any(uv<0)||any(uv>1)) return 0;
                float4 density;
                if(index==0) density=SAMPLE_TEXTURE3D_LOD(_ModernTorchSteam0,sampler_ModernTorchSteam0,uv,0);
                else if(index==1) density=SAMPLE_TEXTURE3D_LOD(_ModernTorchSteam1,sampler_ModernTorchSteam1,uv,0);
                else if(index==2) density=SAMPLE_TEXTURE3D_LOD(_ModernTorchSteam2,sampler_ModernTorchSteam2,uv,0);
                else density=SAMPLE_TEXTURE3D_LOD(_ModernTorchSteam3,sampler_ModernTorchSteam3,uv,0);
                float3 edge=min(uv,1-uv)*12;
                return dot(density,float4(1,1,1,1))*smoothstep(0,.8,min(edge.x,min(edge.y,edge.z)));
            }
            float SteamTransmission(float3 target)
            {
                float3 ro=GetCameraPositionWS(), delta=target-ro;
                float lengthToFlame=length(delta);
                if(lengthToFlame<.0001) return 1;
                float3 rd=delta/lengthToFlame;
                float3 inv=sign(rd+1e-8)/max(abs(rd),1e-6);
                float optical=0;
                [unroll] for(int k=0;k<4;k++)
                {
                    float4 volume=_ModernTorchSteamBounds[k];
                    if(volume.w<=0) continue;
                    float3 a=(volume.xyz-ro)*inv, b=(volume.xyz+12-ro)*inv;
                    float3 lo=min(a,b), hi=max(a,b);
                    float near=max(0,max(lo.x,max(lo.y,lo.z)));
                    float far=min(lengthToFlame,min(hi.x,min(hi.y,hi.z)));
                    if(far<=near) continue;
                    float stepSize=(far-near)/12;
                    [unroll] for(int s=0;s<12;s++)
                    {
                        float3 p=ro+rd*(near+(s+.5)*stepSize);
                        float3 samplePosition; float multiplier,detail;
                        ModernSteamOptics(p,_ModernTorchSteamClock[k],samplePosition,multiplier,detail);
                        float3 uv=(samplePosition-volume.xyz)/12;
                        optical+=SteamDensity(k,uv)*multiplier
                            *ModernSteamBoundary(p,volume.xyz,12)*stepSize*150*volume.w;
                    }
                }
                return exp(-optical);
            }
            half4 Frag(V i):SV_Target
            {
                float2 p=i.uv*2-1;
                float t=_Time.y;
                float grain=Noise(float2(i.uv.x*4+_FlameSeed,i.uv.y*4-t*1.8));
                float fine=Noise(float2(i.uv.x*9-_FlameSeed*.7,i.uv.y*9-t*2.9));
                float density=grain*.7+fine*.3;
                float height=.82+.10*sin(t*3.3+_FlameSeed)+.06*sin(t*7.1+_FlameSeed*1.9);
                float y=saturate(i.uv.y/height);
                float center=p.x+sin(y*8-t*4.2+_FlameSeed)*.22*y*y+(density-.5)*.5*y;
                // A narrow root intersects the fuel; the main tongue swells then
                // tapers. An offset shorter tongue breaks up its silhouette as it
                // rises. Both share the same advected detail and bounded sheets.
                float width=.18*(1-smoothstep(0,.25,y))+.68*max(0,sin(PI*y))*sqrt(1-y);
                width*=lerp(.65,1.15,density);
                float edgeDistance=width-abs(center);
                float feather=max(.055,fwidth(edgeDistance)*1.25);
                float body=smoothstep(-feather,feather,edgeDistance)*(1-smoothstep(.82,1,y));
                float sideHeight=.50+.09*sin(t*4.3+_FlameSeed*.8);
                float sideY=saturate(i.uv.y/sideHeight);
                float sideCenter=p.x+.25*sin(t*2.7+_FlameSeed)+.24*sin(sideY*6-t*3.8+_FlameSeed)*sideY;
                float sideWidth=(.12*(1-smoothstep(0,.2,sideY))+.35*max(0,sin(PI*sideY))*sqrt(1-sideY))*lerp(.7,1.1,density);
                float sideEdge=sideWidth-abs(sideCenter);
                float sideFeather=max(.055,fwidth(sideEdge)*1.25);
                float side=smoothstep(-sideFeather,sideFeather,sideEdge)*(1-smoothstep(.78,1,sideY));
                float opacity=max(body,side*.8)*(.48+.48*smoothstep(.15,.8,density));

                // Keep most of the flame in a restrained orange range. Only a small
                // pocket above the fuel reaches white-hot HDR, preserving shape under
                // ACES/bloom instead of clipping the entire card to neon red/yellow.
                float core=exp(-center*center*70-(y-.19)*(y-.19)*90)*smoothstep(.2,.65,density);
                half3 fire=lerp(half3(.55,.09,.012),half3(2.0,.72,.14),density);
                fire*=lerp(.55,1,smoothstep(.025,.16,y));
                fire=lerp(fire,half3(4.2,3.0,1.45),core);

                // The mesh has three planes separated by 60 degrees. Normalize their
                // facing weights so overlapping sheets do not multiply brightness;
                // the strongest-facing sheet carries most detail, edge-on ones fade.
                float2 normal=normalize(i.normal.xz+float2(.000001,0));
                float2 view=GetWorldSpaceNormalizeViewDir(i.world).xz;
                float2 next=float2(normal.x*.5-normal.y*.8660254,normal.x*.8660254+normal.y*.5);
                float2 last=float2(-normal.x*.5-normal.y*.8660254,normal.x*.8660254-normal.y*.5);
                float3 facing=abs(float3(dot(normal,view),dot(next,view),dot(last,view)));
                facing*=facing; facing*=facing;
                float weight=facing.x/max(dot(facing,1),.00001);
                float alpha=opacity*weight;
                float emberShape=1-smoothstep(.75,1,length(p));
                half3 ember=lerp(half3(.18,.008,.001),half3(1.4,.14,.008),density);
                fire=lerp(fire,ember,i.ember);
                alpha=lerp(alpha,emberShape,i.ember);
                alpha*=SteamTransmission(i.world);
                clip(alpha-.001);
                return half4(MixFog(fire,i.fog),alpha);
            }
            ENDHLSL
        }
    }
    FallBack Off
}
