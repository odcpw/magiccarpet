Shader "MagicCarpet/ModernSky"
{
    Properties
    {
        _ZenithColor ("Zenith", Color) = (0.025, 0.055, 0.11, 1)
        _HorizonColor ("Horizon", Color) = (0.23, 0.27, 0.31, 1)
        _MoonColor ("Moonlight", Color) = (0.82, 0.88, 0.78, 1)
        _MoonDirection ("Moon direction", Vector) = (-0.38, 0.48, 0.79, 0)
        _CloudAmount ("Cloud amount", Range(0,1)) = 0.58
        _CloudNoise ("Cloud lattice", 3D) = "white" {}
        _UseCloudNoise ("Use cloud lattice", Float) = 0
        _Daylight ("Daylight", Range(0,1)) = 0
    }
    SubShader
    {
        Tags { "RenderPipeline"="UniversalPipeline" "Queue"="Background" "RenderType"="Background" "PreviewType"="Skybox" }
        Cull Off ZWrite Off
        Pass
        {
            HLSLPROGRAM
            #pragma target 3.5
            #pragma vertex Vert
            #pragma fragment Frag
            #include "Packages/com.unity.render-pipelines.universal/ShaderLibrary/Core.hlsl"
            CBUFFER_START(UnityPerMaterial)
                half4 _ZenithColor, _HorizonColor, _MoonColor;
                float4 _MoonDirection;
                float _CloudAmount, _UseCloudNoise, _Daylight;
            CBUFFER_END
            TEXTURE3D(_CloudNoise); SAMPLER(sampler_CloudNoise);
            float4 _ModernCloudWindOffset;
            // Supplied by the presenter from real nearby buildings/flames.
            // Position.w is the world-space footprint radius; color.rgb is radiance.
            int _ModernGlowCount;
            float4 _ModernGlowPosition[8];
            float4 _ModernGlowColor[8];
            struct Attributes { float4 positionOS:POSITION; };
            struct Varyings { float4 positionCS:SV_POSITION; float3 direction:TEXCOORD0; };
            Varyings Vert(Attributes v)
            {
                Varyings o; o.positionCS=TransformObjectToHClip(v.positionOS.xyz);
                o.direction=v.positionOS.xyz; return o;
            }
            // Identical integer corners must hash identically across cells.
            float Hash(int3 p)
            {
                uint3 bits=asuint(p);
                uint h=bits.x*0x8da6b343u ^ bits.y*0xd8163841u ^ bits.z*0xcb1ab31fu ^ 0x9e3779b9u;
                h^=h>>16; h*=0x7feb352du; h^=h>>15; h*=0x846ca68bu; h^=h>>16;
                return (h & 0x00ffffffu)*(1.0/16777216.0);
            }
            float Noise(float3 p)
            {
                int3 i=(int3)floor(p); float3 f=frac(p); f=f*f*f*(f*(f*6-15)+10);
                // Quintic interpolation is retained. Texel centers correspond
                // to integer lattice corners; the 128-cell volume wraps smoothly.
                if (_UseCloudNoise > 0.5)
                    return SAMPLE_TEXTURE3D_LOD(_CloudNoise,sampler_CloudNoise,
                        (float3(i)+f+64.5)/128.0,0).r;
                float a=lerp(Hash(i),Hash(i+int3(1,0,0)),f.x);
                float b=lerp(Hash(i+int3(0,1,0)),Hash(i+int3(1,1,0)),f.x);
                float c=lerp(Hash(i+int3(0,0,1)),Hash(i+int3(1,0,1)),f.x);
                float d=lerp(Hash(i+int3(0,1,1)),Hash(i+int3(1,1,1)),f.x);
                return lerp(lerp(a,b,f.y),lerp(c,d,f.y),f.z);
            }
            // Finite layer above the highest NightSpire terrain. World-space
            // coordinates let the field respond to flight rather than sticking
            // to the camera; sampling fades at the finite horizon distance.
            float CloudProfile(float height)
            {
                return smoothstep(0.0,0.18,height)*(1-smoothstep(0.68,1.0,height));
            }
            float CloudDensity(float3 positionWS, float fineWeight)
            {
                float height=(positionWS.y-260.0)/120.0;
                if (height <= 0 || height >= 1) return 0;
                float3 q=float3(positionWS.x*0.006,positionWS.y*0.012,positionWS.z*0.006);
                q.xz-=_ModernCloudWindOffset.xy*0.006;
                float baseShape=Noise(q)*0.58+Noise(q*2.03+11.7)*0.28;
                float detail=lerp(0.5,Noise(q*4.11-6.2),fineWeight);
                float shape=baseShape+detail*0.14;
                float weather=Noise(float3((positionWS.xz-_ModernCloudWindOffset.xy)*0.0013,17.0).xzy);
                float threshold=lerp(0.53,0.37,saturate(_CloudAmount))+(0.5-weather)*0.08+0.03;
                float erosion=(1-detail)*0.04;
                return saturate((shape-threshold-erosion)*4.0)*CloudProfile(height)*saturate(_CloudAmount*2);
            }
            // Lower-frequency density is sufficient for the single moonward
            // self-shadow sample and avoids another full-detail march.
            float CloudLightDensity(float3 positionWS)
            {
                float height=(positionWS.y-260.0)/120.0;
                if (height <= 0 || height >= 1) return 0;
                float3 q=float3(positionWS.x*0.006,positionWS.y*0.012,positionWS.z*0.006);
                q.xz-=_ModernCloudWindOffset.xy*0.006;
                float shape=Noise(q)*0.67+Noise(q*2.03+11.7)*0.33;
                float weather=Noise(float3((positionWS.xz-_ModernCloudWindOffset.xy)*0.0013,17.0).xzy);
                float threshold=lerp(0.53,0.37,saturate(_CloudAmount))+(0.5-weather)*0.08+0.03;
                return saturate((shape-threshold-0.02)*4.0)*CloudProfile(height)*saturate(_CloudAmount*2);
            }
            // Equirectangular angular cells with wrapped longitude. The random
            // point stays inside its cell, avoiding cubic-shell intersections
            // and changing star identity as the camera rotates. Analytic angular
            // derivatives avoid a huge fwidth at the longitude seam.
            float Stars(float3 ray)
            {
                const float longitudeCells=512.0, latitudeCells=256.0;
                float horizontal2=max(dot(ray.xz,ray.xz),1e-7);
                float longitude=atan2(ray.z,ray.x);
                float latitude=asin(clamp(ray.y,-1.0,1.0));
                float2 grid=float2((longitude*(0.5/PI)+0.5)*longitudeCells,
                    (latitude/PI+0.5)*latitudeCells);
                int2 cell=(int2)floor(grid);
                cell.x=(cell.x % 512 + 512) % 512;
                float seed=Hash(int3(cell,43));
                float2 center=0.25+0.5*float2(Hash(int3(cell,71)),Hash(int3(cell,97)));
                float3 rx=ddx(ray), ry=ddy(ray);
                float2 lonDerivatives=float2(ray.x*rx.z-ray.z*rx.x,
                    ray.x*ry.z-ray.z*ry.x)/horizontal2;
                float2 latDerivatives=float2(rx.y,ry.y)*rsqrt(horizontal2);
                float2 footprint=float2((abs(lonDerivatives.x)+abs(lonDerivatives.y))*(0.5/PI)*longitudeCells,
                    (abs(latDerivatives.x)+abs(latDerivatives.y))*(1.0/PI)*latitudeCells);
                footprint=clamp(footprint,0.002,0.15);
                float pixelDistance=length((frac(grid)-center)/footprint);
                float starProfile=1-smoothstep(0.35,1.15,pixelDistance);
                float poleFade=1-smoothstep(0.975,0.999,abs(ray.y));
                return starProfile*step(0.994,seed)*poleFade*lerp(0.55,1.0,saturate((seed-0.994)/0.006));
            }
            // A bounded per-pixel sum: the cloud contribution is evaluated at
            // the slab midpoint once, not inside the 20-step density march.
            void SettlementGlow(float3 ray, float3 cloudPosition, out float3 horizon, out float3 cloud)
            {
                horizon=0; cloud=0;
                [loop] for (int j=0; j<min(max(_ModernGlowCount,0),8); ++j)
                {
                    float3 position=_ModernGlowPosition[j].xyz;
                    float radius=max(_ModernGlowPosition[j].w,1.0);
                    float2 fromCamera=position.xz-_WorldSpaceCameraPos.xz;
                    float distanceToGlow=length(fromCamera);
                    float2 bearing=fromCamera/max(distanceToGlow,0.001);
                    float2 horizontalRay=ray.xz*rsqrt(max(dot(ray.xz,ray.xz),1e-7));
                    float directional=pow(saturate(dot(horizontalRay,bearing)),12.0);
                    float nearby=1-smoothstep(radius*0.25,radius*2.0,distanceToGlow);
                    float cameraFade=rcp(1.0+pow(distanceToGlow/(radius*4.0),2.0));
                    float horizonShape=exp(-abs(ray.y)*12.0)*lerp(directional,1.0,nearby)*cameraFade;
                    float2 cloudDelta=(cloudPosition.xz-position.xz)/radius;
                    float cloudShape=exp2(-dot(cloudDelta,cloudDelta)*1.442695);
                    float3 radiance=max(_ModernGlowColor[j].rgb,0);
                    horizon+=radiance*horizonShape;
                    cloud+=radiance*cloudShape;
                }
            }
            half4 Frag(Varyings i):SV_Target
            {
                float3 ray=normalize(i.direction), moon=normalize(_MoonDirection.xyz);
                float altitude=saturate(ray.y);
                half3 sky=lerp(_HorizonColor.rgb,_ZenithColor.rgb,pow(altitude,0.48));
                sky += half3(0.018,0.025,0.036)*exp(-abs(ray.y-0.035)*14);
                float moonDot=saturate(dot(ray,moon));
                float moonDistance=length(ray-moon);
                float moonAA=max(fwidth(moonDistance),0.00001);
                float diskRadius=lerp(.012,.008,_Daylight);
                float disk=1-smoothstep(diskRadius-moonAA,diskRadius+moonAA,moonDistance);
                float halo=pow(moonDot,96)*0.22 + pow(moonDot,12)*0.035;
                float lunarSurface=1;
                if (disk > 0.001 && _Daylight < .1)
                {
                    // A stable world-oriented basis for broad dark maria and a
                    // gentle full-moon limb falloff, evaluated only on the disk.
                    float3 referenceUp=abs(moon.y) < 0.98 ? float3(0,1,0) : float3(0,0,1);
                    float3 moonRight=normalize(cross(referenceUp,moon));
                    float3 moonUp=cross(moon,moonRight);
                    float2 uv=float2(dot(ray,moonRight),dot(ray,moonUp))/diskRadius;
                    float mariaA=1-smoothstep(0.65,1.0,length((uv-float2(-0.22,0.23))/float2(0.38,0.30)));
                    float mariaB=1-smoothstep(0.65,1.0,length((uv-float2(0.27,0.10))/float2(0.25,0.34)));
                    float mariaC=1-smoothstep(0.65,1.0,length((uv-float2(-0.02,-0.35))/float2(0.34,0.19)));
                    float albedo=1-0.20*saturate(mariaA*0.9+mariaB*0.75+mariaC*0.65);
                    float curvature=sqrt(saturate(1-dot(uv,uv)));
                    lunarSurface=albedo*(0.82+0.18*curvature);
                }
                sky += _MoonColor.rgb*(disk*lerp(1.8,5.0,_Daylight)*lunarSurface+halo);
                sky += half3(0.55,0.64,0.78)*Stars(ray)*smoothstep(0.04,0.3,altitude)*(1-disk)*(1-_Daylight);
                float cloudMidDistance=clamp((320.0-_WorldSpaceCameraPos.y)/max(ray.y,0.001),0.0,2400.0);
                float3 horizonGlow,cloudGlow;
                SettlementGlow(ray,_WorldSpaceCameraPos+ray*cloudMidDistance,horizonGlow,cloudGlow);
                sky += horizonGlow;
                // Intersect a finite, horizontal world-space slab. The range
                // cap and horizon fade avoid the near-horizontal singularity.
                if (abs(ray.y) > 0.001 && _CloudAmount > 0.001)
                {
                    float3 origin=_WorldSpaceCameraPos;
                    float hitBottom=(260.0-origin.y)/ray.y;
                    float hitTop=(380.0-origin.y)/ray.y;
                    float start=max(0.0,min(hitBottom,hitTop));
                    float end=min(2400.0,max(hitBottom,hitTop));
                    float span=max(0.0,end-start);
                    float horizonFade=smoothstep(0.04,0.15,abs(ray.y))*smoothstep(0.0,250.0,span);
                    if (span > 0 && horizonFade > 0)
                    {
                        const int steps=20;
                        float stepLength=span/steps;
                        // Continuous angular jitter breaks aligned slice bands
                        // without per-frame random flicker.
                        float jitter=lerp(0.18,0.82,Noise(ray*13.0+31.7));
                        float fineWeight=1-smoothstep(0.35,1.1,stepLength*0.006*4.11);
                        float transmittance=1.0;
                        float3 scattering=0;
                        // Forward scattering brightens cloud bodies toward the
                        // same moon direction as the disk; no outline threshold.
                        float phase=0.38+0.40*pow(moonDot,8)+0.65*pow(moonDot,64);
                        [loop] for (int stepIndex=0; stepIndex<steps; ++stepIndex)
                        {
                            float distanceAlongRay=start+(stepIndex+jitter)*stepLength;
                            float3 samplePosition=origin+ray*distanceAlongRay;
                            float density=CloudDensity(samplePosition,fineWeight)*horizonFade;
                            if (density > 0.001)
                            {
                                float extinction=density*stepLength*0.018;
                                float opacity=1-exp(-extinction);
                                float lightDensity=CloudLightDensity(samplePosition+moon*70.0);
                                float lightTransmittance=exp(-lightDensity*1.26);
                                float height=saturate((samplePosition.y-260.0)/120.0);
                                float3 ambient=lerp(_HorizonColor.rgb*.38,_ZenithColor.rgb*.8+_HorizonColor.rgb*.22,height);
                                float3 light=ambient+_MoonColor.rgb*(lerp(.10,.65,_Daylight)*phase*lightTransmittance)
                                    +cloudGlow*0.25*(1-height);
                                scattering+=transmittance*opacity*light;
                                transmittance*=1-opacity;
                                if (transmittance < 0.02) break;
                            }
                        }
                        sky=sky*transmittance+scattering;
                    }
                }
                return half4(sky,1);
            }
            ENDHLSL
        }
    }
    FallBack Off
}
