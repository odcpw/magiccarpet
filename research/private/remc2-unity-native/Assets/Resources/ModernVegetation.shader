Shader "MagicCarpet/ModernVegetation"
{
    Properties
    {
        _WindStrength ("Wind strength", Float) = 0.16
        _WindSpeed ("Wind speed", Float) = 1.0
        _Smoothness ("Smoothness", Range(0,1)) = 0.15
        _UseTextures ("Use textures", Range(0,1)) = 0
        _BaseMap ("Foliage albedo", 2D) = "white" {}
        _AlphaMap ("Foliage opacity (R)", 2D) = "white" {}
        _NormalMap ("Foliage normal (RGB)", 2D) = "bump" {}
        _RoughnessMap ("Foliage roughness (R)", 2D) = "white" {}
        _UseAlphaMap ("Use separate opacity", Float) = 0
        _UseNormalMap ("Use normal map", Float) = 0
        _UseRoughnessMap ("Use roughness map", Float) = 0
        _Cutoff ("Alpha cutoff", Range(0,1)) = 0.4
        _WindUseHeight ("Use object height for wind", Float) = 0
        _WindMinY ("Wind base height", Float) = 0
        _WindInvHeight ("Inverse wind height", Float) = 1
    }
    SubShader
    {
        Tags { "RenderPipeline"="UniversalPipeline" "RenderType"="Opaque" "Queue"="Geometry" }
        Cull Off
        HLSLINCLUDE
        #include "Packages/com.unity.render-pipelines.universal/ShaderLibrary/Core.hlsl"
        #include "Packages/com.unity.render-pipelines.universal/ShaderLibrary/Lighting.hlsl"
        TEXTURE2D(_BaseMap); SAMPLER(sampler_BaseMap);
        TEXTURE2D(_AlphaMap); SAMPLER(sampler_AlphaMap);
        TEXTURE2D(_NormalMap); SAMPLER(sampler_NormalMap);
        TEXTURE2D(_RoughnessMap); SAMPLER(sampler_RoughnessMap);
        CBUFFER_START(UnityPerMaterial)
            float4 _BaseMap_ST;
            float _WindStrength, _WindSpeed, _Smoothness, _UseTextures;
            float _UseAlphaMap, _UseNormalMap, _UseRoughnessMap, _Cutoff;
            float _WindUseHeight, _WindMinY, _WindInvHeight;
        CBUFFER_END
        // Global diagnostic switch; zero for normal presentation.
        float _ModernVegetationDebug;
        float WindWeight(float3 positionOS, float uvWeight)
        {
            return lerp(uvWeight, saturate((positionOS.y-_WindMinY)*_WindInvHeight), saturate(_WindUseHeight));
        }
        half4 FoliageSample(float2 uv, half4 vertexColor)
        {
            if (_UseTextures < 0.5) return vertexColor;
            half4 texel=SAMPLE_TEXTURE2D(_BaseMap,sampler_BaseMap,uv);
            if (_UseAlphaMap > 0.5) texel.a *= SAMPLE_TEXTURE2D(_AlphaMap,sampler_AlphaMap,uv).r;
            clip(texel.a-_Cutoff);
            return texel*vertexColor;
        }
        float3 WindPosition(float3 positionWS, float weight)
        {
            float phase = dot(positionWS.xz, float2(0.14,0.09)) + _Time.y*_WindSpeed;
            float sway = sin(phase) + 0.35*sin(phase*1.83+positionWS.y*0.21);
            positionWS.xz += float2(0.85,0.4)*sway*_WindStrength*saturate(weight);
            return positionWS;
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
            struct Attributes { float4 positionOS:POSITION; float3 normalOS:NORMAL; float4 tangentOS:TANGENT; half4 color:COLOR; float2 uv:TEXCOORD0; UNITY_VERTEX_INPUT_INSTANCE_ID };
            struct Varyings { float4 positionCS:SV_POSITION; float3 positionWS:TEXCOORD0; half3 normalWS:TEXCOORD1; half4 color:COLOR; half fog:TEXCOORD2; float2 uv:TEXCOORD3; half4 tangentWS:TEXCOORD4; UNITY_VERTEX_INPUT_INSTANCE_ID };
            Varyings Vert(Attributes v)
            {
                UNITY_SETUP_INSTANCE_ID(v); Varyings o;
                UNITY_TRANSFER_INSTANCE_ID(v,o);
                o.positionWS=WindPosition(TransformObjectToWorld(v.positionOS.xyz),WindWeight(v.positionOS.xyz,v.uv.y));
                o.positionCS=TransformWorldToHClip(o.positionWS);
                o.normalWS=TransformObjectToWorldNormal(v.normalOS); o.color=v.color;
                o.tangentWS=half4(TransformObjectToWorldDir(v.tangentOS.xyz),v.tangentOS.w*GetOddNegativeScale());
                o.uv=TRANSFORM_TEX(v.uv,_BaseMap);
                o.fog=ComputeFogFactor(o.positionCS.z); return o;
            }
            half4 Frag(Varyings i, FRONT_FACE_TYPE face:FRONT_FACE_SEMANTIC):SV_Target
            {
                UNITY_SETUP_INSTANCE_ID(i);
                half4 texel=FoliageSample(i.uv,i.color);
                half3 n=normalize(i.normalWS) * (IS_FRONT_VFACE(face,1.0,-1.0));
                if (_UseTextures > 0.5 && _UseNormalMap > 0.5)
                {
                    half3 tangent=normalize(i.tangentWS.xyz);
                    half3 bitangent=normalize(cross(n,tangent))*i.tangentWS.w;
                    half3 normalTS=SAMPLE_TEXTURE2D(_NormalMap,sampler_NormalMap,i.uv).rgb*2-1;
                    n=normalize(tangent*normalTS.x + bitangent*normalTS.y + n*normalTS.z);
                }
                InputData input=(InputData)0; input.positionWS=i.positionWS; input.normalWS=n;
                input.viewDirectionWS=GetWorldSpaceNormalizeViewDir(i.positionWS);
                input.shadowCoord=TransformWorldToShadowCoord(i.positionWS);
                input.bakedGI=SampleSH(n); input.normalizedScreenSpaceUV=GetNormalizedScreenSpaceUV(i.positionCS);
                input.shadowMask=half4(1,1,1,1);
                if (_ModernVegetationDebug > 0.5 && _ModernVegetationDebug < 1.5)
                    return half4(saturate(input.bakedGI),1);
                if (_ModernVegetationDebug > 2.5 && _ModernVegetationDebug < 3.5)
                {
                    Light direct=GetMainLight(input.shadowCoord);
                    half intensity=saturate(dot(n,direct.direction))*direct.distanceAttenuation*direct.shadowAttenuation;
                    return half4(intensity,intensity,intensity,1);
                }
                SurfaceData surface=(SurfaceData)0; surface.albedo=texel.rgb; surface.metallic=0;
                if (_ModernVegetationDebug > 1.5 && _ModernVegetationDebug < 2.5)
                    surface.albedo=half3(0.5,0.5,0.5);
                surface.specular=half3(0.04,0.04,0.04); surface.smoothness=_Smoothness;
                if (_UseTextures > 0.5 && _UseRoughnessMap > 0.5)
                    surface.smoothness=1-SAMPLE_TEXTURE2D(_RoughnessMap,sampler_RoughnessMap,i.uv).r;
                surface.occlusion=1; surface.alpha=1; surface.normalTS=half3(0,0,1);
                half4 result=UniversalFragmentPBR(input,surface); result.rgb=MixFog(result.rgb,i.fog); return result;
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
            struct ShadowAttributes { float4 positionOS:POSITION; float3 normalOS:NORMAL; float2 uv:TEXCOORD0; UNITY_VERTEX_INPUT_INSTANCE_ID };
            struct ShadowVaryings { float4 positionCS:SV_POSITION; float2 uv:TEXCOORD0; };
            ShadowVaryings ShadowVert(ShadowAttributes v)
            {
                UNITY_SETUP_INSTANCE_ID(v); ShadowVaryings o;
                float3 world=WindPosition(TransformObjectToWorld(v.positionOS.xyz),WindWeight(v.positionOS.xyz,v.uv.y));
                float3 normal=TransformObjectToWorldNormal(v.normalOS);
                #if _CASTING_PUNCTUAL_LIGHT_SHADOW
                    float3 lightDirection=normalize(_LightPosition-world);
                #else
                    float3 lightDirection=_LightDirection;
                #endif
                o.positionCS=ApplyShadowClamping(TransformWorldToHClip(ApplyShadowBias(world,normal,lightDirection)));
                o.uv=TRANSFORM_TEX(v.uv,_BaseMap);
                return o;
            }
            half4 ShadowFrag(ShadowVaryings i):SV_Target { FoliageSample(i.uv,half4(1,1,1,1)); return 0; }
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
            struct DepthAttributes { float4 positionOS:POSITION; float2 uv:TEXCOORD0; UNITY_VERTEX_INPUT_INSTANCE_ID };
            struct DepthVaryings { float4 positionCS:SV_POSITION; float2 uv:TEXCOORD0; };
            DepthVaryings DepthVert(DepthAttributes v)
            {
                UNITY_SETUP_INSTANCE_ID(v); DepthVaryings o;
                float3 world=WindPosition(TransformObjectToWorld(v.positionOS.xyz),WindWeight(v.positionOS.xyz,v.uv.y));
                o.positionCS=TransformWorldToHClip(world); o.uv=TRANSFORM_TEX(v.uv,_BaseMap); return o;
            }
            half4 DepthFrag(DepthVaryings i):SV_Target { FoliageSample(i.uv,half4(1,1,1,1)); return 0; }
            ENDHLSL
        }
    }
    FallBack Off
}
