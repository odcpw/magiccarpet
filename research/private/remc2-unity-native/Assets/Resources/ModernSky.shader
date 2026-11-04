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
                float _CloudAmount, _UseCloudNoise;
            CBUFFER_END
            TEXTURE3D(_CloudNoise); SAMPLER(sampler_CloudNoise);
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
                q.xz+=float2(_Time.y*0.006,_Time.y*0.0025);
                float baseShape=Noise(q)*0.58+Noise(q*2.03+11.7)*0.28;
                float detail=lerp(0.5,Noise(q*4.11-6.2),fineWeight);
                float shape=baseShape+detail*0.14;
                float weather=Noise(float3(positionWS.x*0.0013,17.0,positionWS.z*0.0013));
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
                q.xz+=float2(_Time.y*0.006,_Time.y*0.0025);
                float shape=Noise(q)*0.67+Noise(q*2.03+11.7)*0.33;
                float weather=Noise(float3(positionWS.x*0.0013,17.0,positionWS.z*0.0013));
                float threshold=lerp(0.53,0.37,saturate(_CloudAmount))+(0.5-weather)*0.08+0.03;
                return saturate((shape-threshold-0.02)*4.0)*CloudProfile(height)*saturate(_CloudAmount*2);
            }
            half4 Frag(Varyings i):SV_Target
            {
                float3 ray=normalize(i.direction), moon=normalize(_MoonDirection.xyz);
                float altitude=saturate(ray.y);
                half3 sky=lerp(_HorizonColor.rgb,_ZenithColor.rgb,pow(altitude,0.48));
                sky += half3(0.075,0.055,0.045)*exp(-abs(ray.y-0.035)*14);
                float moonDot=saturate(dot(ray,moon));
                float disk=smoothstep(0.9995,0.9997,moonDot);
                float halo=pow(moonDot,96)*0.22 + pow(moonDot,12)*0.035;
                sky += _MoonColor.rgb*(disk*1.8+halo);
                int3 starCell=(int3)floor(ray*620); float starHash=Hash(starCell);
                float3 starOffset=frac(ray*620)-0.5;
                float star=(1-smoothstep(0.04,0.24,length(starOffset)))*step(0.997,starHash);
                sky += half3(0.55,0.64,0.78)*star*smoothstep(0.04,0.3,altitude);
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
                        float phase=0.38+0.40*pow(moonDot,8);
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
                                float3 ambient=lerp(float3(0.10,0.13,0.18),float3(0.19,0.23,0.29),height);
                                float3 light=ambient+_MoonColor.rgb*(0.12*phase*lightTransmittance);
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
