Shader "MagicCarpet/ModernTerrain"
{
    Properties
    {
        _RockColor ("Rock", Color) = (0.27, 0.26, 0.25, 1)
        _SoilColor ("Soil", Color) = (0.25, 0.19, 0.13, 1)
        _MossColor ("Moss", Color) = (0.16, 0.25, 0.13, 1)
        _DetailScale ("Detail scale", Float) = 1
        _UseTextures ("Use surface textures", Range(0,1)) = 0
        _TextureScale ("Texture repeats per world unit", Float) = 0.25
        _RockAlbedo ("Rock albedo", 2D) = "white" {}
        _GroundAlbedo ("Ground albedo", 2D) = "white" {}
        _RockNormal ("Rock normal (RGB)", 2D) = "bump" {}
        _GroundNormal ("Ground normal (RGB)", 2D) = "bump" {}
        _RockRoughness ("Rock roughness (R)", 2D) = "white" {}
        _GroundRoughness ("Ground roughness (R)", 2D) = "white" {}
        _UseMaterialEndpoints ("Skip unused pure-material samples", Float) = 1
    }
    SubShader
    {
        Tags { "RenderPipeline"="UniversalPipeline" "RenderType"="Opaque" "Queue"="Geometry" }
        Pass
        {
            Name "ForwardLit"
            Tags { "LightMode"="UniversalForward" }
            HLSLPROGRAM
            #pragma target 3.5
            #pragma vertex Vert
            #pragma fragment Frag
            #pragma multi_compile _ _CLUSTER_LIGHT_LOOP
            #pragma multi_compile _ _MAIN_LIGHT_SHADOWS _MAIN_LIGHT_SHADOWS_CASCADE _MAIN_LIGHT_SHADOWS_SCREEN
            #pragma multi_compile_fragment _ _SHADOWS_SOFT _SHADOWS_SOFT_LOW _SHADOWS_SOFT_MEDIUM _SHADOWS_SOFT_HIGH
            #pragma multi_compile_fog
            #include "Packages/com.unity.render-pipelines.universal/ShaderLibrary/Core.hlsl"
            #include "Packages/com.unity.render-pipelines.universal/ShaderLibrary/Lighting.hlsl"
            TEXTURE2D(_RockAlbedo); SAMPLER(sampler_RockAlbedo);
            TEXTURE2D(_GroundAlbedo); SAMPLER(sampler_GroundAlbedo);
            TEXTURE2D(_RockNormal); SAMPLER(sampler_RockNormal);
            TEXTURE2D(_GroundNormal); SAMPLER(sampler_GroundNormal);
            TEXTURE2D(_RockRoughness); SAMPLER(sampler_RockRoughness);
            TEXTURE2D(_GroundRoughness); SAMPLER(sampler_GroundRoughness);
            CBUFFER_START(UnityPerMaterial)
                half4 _RockColor, _SoilColor, _MossColor;
                float _DetailScale, _UseTextures, _TextureScale, _UseMaterialEndpoints;
            CBUFFER_END
            struct Attributes { float4 positionOS : POSITION; float3 normalOS : NORMAL; half4 color : COLOR; };
            struct Varyings { float4 positionCS : SV_POSITION; float3 positionWS : TEXCOORD0; half3 normalWS : TEXCOORD1; half4 color : COLOR; half fog : TEXCOORD2; };
            Varyings Vert(Attributes v)
            {
                Varyings o; VertexPositionInputs p = GetVertexPositionInputs(v.positionOS.xyz);
                o.positionCS = p.positionCS; o.positionWS = p.positionWS;
                o.normalWS = TransformObjectToWorldNormal(v.normalOS); o.color = v.color;
                o.fog = ComputeFogFactor(p.positionCS.z); return o;
            }
            // Shared lattice corners must have the same value across cells.
            float Hash(int2 p)
            {
                uint2 bits=asuint(p);
                uint h=bits.x*0x8da6b343u ^ bits.y*0xd8163841u ^ 0xcb1ab31fu;
                h^=h>>16; h*=0x7feb352du; h^=h>>15; h*=0x846ca68bu; h^=h>>16;
                return (h & 0x00ffffffu)*(1.0/16777216.0);
            }
            float Noise(float2 p)
            {
                int2 i=(int2)floor(p); float2 f=frac(p); f=f*f*(3.0-2.0*f);
                return lerp(lerp(Hash(i), Hash(i+int2(1,0)), f.x), lerp(Hash(i+int2(0,1)), Hash(i+int2(1,1)), f.x), f.y);
            }
            float Detail(float2 p)
            {
                return 0.57*Noise(p) + 0.28*Noise(p*2.07+13.1) + 0.15*Noise(p*4.13-7.2);
            }
            float2 AlternateRockUV(float2 uv)
            {
                return float2(-uv.y,uv.x)*0.71+float2(13.1,-7.3);
            }
            float2 AlternateRockGradient(float2 gradient)
            {
                return float2(-gradient.y,gradient.x)*0.71;
            }
            half2 AlternateRockNormal(half2 encoded)
            {
                half2 n=encoded*2-1;
                return half2(n.y,-n.x)*0.71;
            }
            half4 Frag(Varyings i) : SV_Target
            {
                float3 n = normalize(i.normalWS), p = i.positionWS * _DetailScale;
                float3 weights = pow(abs(n), 4.0); weights /= max(dot(weights, float3(1,1,1)), 0.0001);
                float detail = Detail(p.yz * 0.65)*weights.x + Detail(p.xz * 0.65)*weights.y + Detail(p.xy * 0.65)*weights.z;
                // Attenuate sub-pixel grain at distance to prevent temporal shimmer.
                detail = lerp(0.5, detail, 1-smoothstep(0.12, 0.8, length(fwidth(p))));
                float broad = Noise(p.xz * 0.032) * 0.65 + Noise(p.xz * 0.095) * 0.35;
                float rock = saturate((0.78-n.y)*2.8 + (detail-0.5)*0.45);
                float moss = saturate((n.y-0.48)*2.1) * smoothstep(0.36, 0.69, broad + (detail-0.5)*0.2);
                // Broad continuous shading varies the repeated rock texture
                // without introducing per-pixel speckles or moving the terrain.
                float rockVariation=Noise(p.xz*0.012+p.y*float2(0.006,0.009)+float2(24,17));
                float dampPatch=smoothstep(0.38,0.68,broad+(detail-0.5)*0.08)*saturate((n.y-0.45)*2);
                float wetShore=saturate(i.color.a)*(1-smoothstep(0.35,3.0,max(i.positionWS.y,0.0)))*(1-rock*0.35);
                half3 ground = lerp(_SoilColor.rgb, _MossColor.rgb, moss);
                half3 albedo = lerp(ground, _RockColor.rgb, rock);
                albedo *= lerp(0.67, 1.28, detail) * lerp(0.80, 1.20, broad);
                half smoothness = lerp(0.12, 0.26, rock);
                if (_UseTextures > 0.001)
                {
                    float3 q = i.positionWS * _TextureScale;
                    float2 altX=AlternateRockUV(q.yz), altY=AlternateRockUV(q.xz), altZ=AlternateRockUV(q.xy);
                    // Compute gradients before material-dependent control flow.
                    // Implicit derivatives within a divergent branch could
                    // select different mips at the rock/soil boundary.
                    float3 qdx=ddx(q), qdy=ddy(q);
                    float2 altXdx=AlternateRockGradient(qdx.yz), altXdy=AlternateRockGradient(qdy.yz);
                    float2 altYdx=AlternateRockGradient(qdx.xz), altYdy=AlternateRockGradient(qdy.xz);
                    float2 altZdx=AlternateRockGradient(qdx.xy), altZdy=AlternateRockGradient(qdy.xy);
                    float rockMix=lerp(0.28,0.62,rockVariation);
                    half3 rockTex=0, soilTex=0;
                    half rockR=0, soilR=0;
                    half2 rockNX=0, rockNY=0, rockNZ=0, soilNX=0, soilNY=0, soilNZ=0;
                    // Saturated material weights have exact endpoints. On
                    // those pixels the other material contributes nothing.
                    [branch] if (_UseMaterialEndpoints < 0.5 || rock > 0)
                    {
                        half3 rockX = SAMPLE_TEXTURE2D_GRAD(_RockAlbedo, sampler_RockAlbedo, q.yz,qdx.yz,qdy.yz).rgb;
                        half3 rockY = SAMPLE_TEXTURE2D_GRAD(_RockAlbedo, sampler_RockAlbedo, q.xz,qdx.xz,qdy.xz).rgb;
                        half3 rockZ = SAMPLE_TEXTURE2D_GRAD(_RockAlbedo, sampler_RockAlbedo, q.xy,qdx.xy,qdy.xy).rgb;
                        rockX=lerp(rockX,SAMPLE_TEXTURE2D_GRAD(_RockAlbedo,sampler_RockAlbedo,altX,altXdx,altXdy).rgb,rockMix);
                        rockY=lerp(rockY,SAMPLE_TEXTURE2D_GRAD(_RockAlbedo,sampler_RockAlbedo,altY,altYdx,altYdy).rgb,rockMix);
                        rockZ=lerp(rockZ,SAMPLE_TEXTURE2D_GRAD(_RockAlbedo,sampler_RockAlbedo,altZ,altZdx,altZdy).rgb,rockMix);
                        rockTex = rockX*weights.x + rockY*weights.y + rockZ*weights.z;
                        rockTex *= lerp(half3(0.77,0.82,0.80),half3(1.07,1.02,0.95),rockVariation);
                        rockR = SAMPLE_TEXTURE2D_GRAD(_RockRoughness, sampler_RockRoughness, q.yz,qdx.yz,qdy.yz).r*weights.x
                            + SAMPLE_TEXTURE2D_GRAD(_RockRoughness, sampler_RockRoughness, q.xz,qdx.xz,qdy.xz).r*weights.y
                            + SAMPLE_TEXTURE2D_GRAD(_RockRoughness, sampler_RockRoughness, q.xy,qdx.xy,qdy.xy).r*weights.z;
                        half altRockR=SAMPLE_TEXTURE2D_GRAD(_RockRoughness,sampler_RockRoughness,altX,altXdx,altXdy).r*weights.x
                            + SAMPLE_TEXTURE2D_GRAD(_RockRoughness,sampler_RockRoughness,altY,altYdx,altYdy).r*weights.y
                            + SAMPLE_TEXTURE2D_GRAD(_RockRoughness,sampler_RockRoughness,altZ,altZdx,altZdy).r*weights.z;
                        rockR=lerp(rockR,altRockR,rockMix);
                        // Maps are linear RGB tangent normals, not DXT5nm.
                        rockNX=SAMPLE_TEXTURE2D_GRAD(_RockNormal,sampler_RockNormal,q.yz,qdx.yz,qdy.yz).rg*2-1;
                        rockNY=SAMPLE_TEXTURE2D_GRAD(_RockNormal,sampler_RockNormal,q.xz,qdx.xz,qdy.xz).rg*2-1;
                        rockNZ=SAMPLE_TEXTURE2D_GRAD(_RockNormal,sampler_RockNormal,q.xy,qdx.xy,qdy.xy).rg*2-1;
                        rockNX=lerp(rockNX,AlternateRockNormal(SAMPLE_TEXTURE2D_GRAD(_RockNormal,sampler_RockNormal,altX,altXdx,altXdy).rg),rockMix);
                        rockNY=lerp(rockNY,AlternateRockNormal(SAMPLE_TEXTURE2D_GRAD(_RockNormal,sampler_RockNormal,altY,altYdx,altYdy).rg),rockMix);
                        rockNZ=lerp(rockNZ,AlternateRockNormal(SAMPLE_TEXTURE2D_GRAD(_RockNormal,sampler_RockNormal,altZ,altZdx,altZdy).rg),rockMix);
                    }
                    [branch] if (_UseMaterialEndpoints < 0.5 || rock < 1)
                    {
                        half3 soilX = SAMPLE_TEXTURE2D_GRAD(_GroundAlbedo, sampler_GroundAlbedo, q.yz,qdx.yz,qdy.yz).rgb;
                        half3 soilY = SAMPLE_TEXTURE2D_GRAD(_GroundAlbedo, sampler_GroundAlbedo, q.xz,qdx.xz,qdy.xz).rgb;
                        half3 soilZ = SAMPLE_TEXTURE2D_GRAD(_GroundAlbedo, sampler_GroundAlbedo, q.xy,qdx.xy,qdy.xy).rgb;
                        soilTex = soilX*weights.x + soilY*weights.y + soilZ*weights.z;
                        soilTex *= lerp(half3(0.84,0.85,0.78),half3(0.69,0.79,0.60),dampPatch*0.7);
                        soilR = SAMPLE_TEXTURE2D_GRAD(_GroundRoughness, sampler_GroundRoughness, q.yz,qdx.yz,qdy.yz).r*weights.x
                            + SAMPLE_TEXTURE2D_GRAD(_GroundRoughness, sampler_GroundRoughness, q.xz,qdx.xz,qdy.xz).r*weights.y
                            + SAMPLE_TEXTURE2D_GRAD(_GroundRoughness, sampler_GroundRoughness, q.xy,qdx.xy,qdy.xy).r*weights.z;
                        soilNX=SAMPLE_TEXTURE2D_GRAD(_GroundNormal,sampler_GroundNormal,q.yz,qdx.yz,qdy.yz).rg*2-1;
                        soilNY=SAMPLE_TEXTURE2D_GRAD(_GroundNormal,sampler_GroundNormal,q.xz,qdx.xz,qdy.xz).rg*2-1;
                        soilNZ=SAMPLE_TEXTURE2D_GRAD(_GroundNormal,sampler_GroundNormal,q.xy,qdx.xy,qdy.xy).rg*2-1;
                    }
                    half3 textured = lerp(soilTex, rockTex, rock) * lerp(0.82,1.15,broad);
                    albedo = lerp(albedo, textured, saturate(_UseTextures));
                    smoothness = lerp(smoothness, 1-saturate(lerp(soilR,rockR,rock)), saturate(_UseTextures));
                    half2 nx=lerp(soilNX,rockNX,rock);
                    half2 ny=lerp(soilNY,rockNY,rock);
                    half2 nz=lerp(soilNZ,rockNZ,rock);
                    float3 perturb = float3(0,nx.x,nx.y)*weights.x + float3(ny.x,0,ny.y)*weights.y
                        + float3(nz.x,nz.y,0)*weights.z;
                    perturb -= n*dot(n,perturb);
                    n = normalize(n + perturb * (0.55*saturate(_UseTextures)));
                }
                // Vertex tint is intentionally subtle so authored classifications can grade the landscape.
                albedo *= lerp(half3(1,1,1), i.color.rgb, 0.25);
                albedo *= lerp(1.0,0.72,wetShore);
                smoothness=lerp(smoothness,max(smoothness,0.38),wetShore*0.6);
                // The textured path already has normal maps. For the procedural
                // fallback, derive the tangent plane from geometry rather than a
                // perturbed shading normal, which can make its determinant vanish.
                if (_UseTextures < 0.999)
                {
                    float3 dpdx=ddx(i.positionWS), dpdy=ddy(i.positionWS);
                    float detailDx=ddx(detail), detailDy=ddy(detail);
                    float3 crossNormal=cross(dpdx,dpdy);
                    float area=length(crossNormal);
                    if (area > 0.000001)
                    {
                        float3 geometric=crossNormal/area;
                        geometric *= dot(geometric,n) < 0 ? -1 : 1;
                        float3 r1=cross(dpdy,geometric), r2=cross(geometric,dpdx);
                        float det=dot(dpdx,r1);
                        float3 grad=(r1*detailDx+r2*detailDy)/max(abs(det),0.000001)*sign(det);
                        grad *= min(1.0,4.0/max(length(grad),0.000001));
                        n=normalize(n-grad*(0.18*(1-saturate(_UseTextures))));
                    }
                }
                else
                {
                    // A shallow, filtered material-height field gives broad
                    // strata and fissures without displacing the source mesh.
                    float fissureField=Noise(p.xz*0.095+p.y*float2(0.011,-0.017));
                    float fissure=1-smoothstep(0.018,0.09,abs(fissureField-0.52));
                    float strataPhase=p.y*0.55+Noise(p.xz*0.055)*1.2;
                    float strata=sin(strataPhase);
                    float fissureDx=ddx(fissure), fissureDy=ddy(fissure);
                    float strataDx=ddx(strata), strataDy=ddy(strata);
                    float fissureFade=1-smoothstep(0.03,0.15,fwidth(fissureField));
                    float strataFade=1-smoothstep(0.6,2.5,fwidth(strataPhase));
                    float3 dpdx=ddx(i.positionWS), dpdy=ddy(i.positionWS);
                    float reliefDx=0.16*strataDx*strataFade+0.25*fissureDx*fissureFade;
                    float reliefDy=0.16*strataDy*strataFade+0.25*fissureDy*fissureFade;
                    float fade=1-smoothstep(0.8,3.0,length(fwidth(i.positionWS)));
                    float3 crossNormal=cross(dpdx,dpdy);
                    float area=length(crossNormal);
                    if (area > 0.000001)
                    {
                        float3 geometric=crossNormal/area;
                        geometric *= dot(geometric,n) < 0 ? -1 : 1;
                        float3 r1=cross(dpdy,geometric), r2=cross(geometric,dpdx);
                        float det=dot(dpdx,r1);
                        float3 grad=(r1*reliefDx+r2*reliefDy)/max(abs(det),0.000001)*sign(det);
                        grad*=min(1.0,1.5/max(length(grad),0.000001));
                        n=normalize(n-grad*(rock*fade*0.32));
                    }
                }
                InputData input = (InputData)0; input.positionWS = i.positionWS; input.normalWS = n;
                input.viewDirectionWS = GetWorldSpaceNormalizeViewDir(i.positionWS);
                input.shadowCoord = TransformWorldToShadowCoord(i.positionWS);
                input.bakedGI = SampleSH(n); input.normalizedScreenSpaceUV = GetNormalizedScreenSpaceUV(i.positionCS);
                input.shadowMask = half4(1,1,1,1);
                SurfaceData surface = (SurfaceData)0; surface.albedo = albedo; surface.metallic = 0;
                surface.specular = half3(0.04,0.04,0.04); surface.smoothness = smoothness;
                surface.occlusion = 1; surface.alpha = 1; surface.normalTS = half3(0,0,1);
                half4 result = UniversalFragmentPBR(input, surface); result.rgb = MixFog(result.rgb, i.fog); return result;
            }
            ENDHLSL
        }
        UsePass "Universal Render Pipeline/Lit/ShadowCaster"
        UsePass "Universal Render Pipeline/Lit/DepthOnly"
    }
    FallBack Off
}
