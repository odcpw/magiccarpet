Shader "MagicCarpet/Enhanced/Water"
{
    Properties
    {
        [MainColor] _BaseColor ("Base Color", Color) = (0.008, 0.04, 0.12, 1)
        _ReflectionColor ("Sky Reflection", Color) = (0.025, 0.15, 0.42, 1)
        _FoamColor ("Wave Crest", Color) = (0.08, 0.25, 0.60, 1)
        [HDR] _MoonColor ("Moon Streak Color", Color) = (0.34, 0.58, 1, 1)
        _WaveScale ("Wave Scale", Float) = 0.085
        _WaveSpeed ("Wave Speed", Float) = 0.32
        _WaveStrength ("Wave Normal Strength", Range(0, 1)) = 0.28
        _FresnelStrength ("Fresnel Strength", Range(0, 2)) = 1.30
        _SpecularStrength ("Moon Glint Strength", Range(0, 4)) = 1.75
        _ReflectionStrength ("Reflection Strength", Range(0, 2)) = 1.15
        _SceneReflectionStrength ("Scene Reflection", Range(0, 1)) = 0.72
        _ReflectionDistortion ("Reflection Distortion", Range(0, 0.2)) = 0.035
        _RippleCenter ("Ripple Center", Vector) = (0, 0, 0, 0)
        _RippleStartTime ("Ripple Start Time", Float) = 0
        _RippleStrength ("Ripple Strength", Range(0, 2)) = 0
        _RippleSpeed ("Ripple Speed", Float) = 2.8
        _RippleWidth ("Ripple Width", Float) = 0.65
        _Alpha ("Surface Opacity", Range(0, 1)) = 0.94
    }

    SubShader
    {
        Tags
        {
            "Queue" = "Transparent"
            "RenderType" = "Transparent"
            "RenderPipeline" = "UniversalPipeline"
        }

        Blend SrcAlpha OneMinusSrcAlpha
        ZWrite Off
        Cull Back

        Pass
        {
            Name "WaterForward"
            Tags { "LightMode" = "UniversalForward" }

            HLSLPROGRAM
            #pragma vertex Vert
            #pragma fragment Frag
            #pragma multi_compile_fog

            #include "Packages/com.unity.render-pipelines.universal/ShaderLibrary/Core.hlsl"
            #include "Packages/com.unity.render-pipelines.universal/ShaderLibrary/DeclareOpaqueTexture.hlsl"
            #include "Packages/com.unity.render-pipelines.universal/ShaderLibrary/Lighting.hlsl"

            CBUFFER_START(UnityPerMaterial)
                half4 _BaseColor;
                half4 _ReflectionColor;
                half4 _FoamColor;
                half4 _MoonColor;
                half _WaveScale;
                half _WaveSpeed;
                half _WaveStrength;
                half _FresnelStrength;
                half _SpecularStrength;
                half _ReflectionStrength;
                half _SceneReflectionStrength;
                half _ReflectionDistortion;
                float4 _RippleCenter;
                half _RippleStartTime;
                half _RippleStrength;
                half _RippleSpeed;
                half _RippleWidth;
                half _Alpha;
            CBUFFER_END

            struct Attributes
            {
                float4 positionOS : POSITION;
                float3 normalOS : NORMAL;
            };

            struct Varyings
            {
                float4 positionHCS : SV_POSITION;
                float3 positionWS : TEXCOORD0;
                half3 normalWS : TEXCOORD1;
                float4 screenPos : TEXCOORD2;
            };

            Varyings Vert(Attributes input)
            {
                Varyings output;
                VertexPositionInputs positionInputs = GetVertexPositionInputs(input.positionOS.xyz);
                VertexNormalInputs normalInputs = GetVertexNormalInputs(input.normalOS);
                output.positionHCS = positionInputs.positionCS;
                output.positionWS = positionInputs.positionWS;
                output.normalWS = normalInputs.normalWS;
                output.screenPos = ComputeScreenPos(output.positionHCS);
                return output;
            }

            float Wave(float2 position, float2 direction, float scale, float time)
            {
                return sin(dot(position * scale, direction) + time);
            }

            half3 WaveNormal(float2 position, half3 baseNormal)
            {
                float time = _Time.y;
                float2 directionA = normalize(float2(1.0, 0.35));
                float2 directionB = normalize(float2(-0.25, 1.0));
                float epsilon = 0.16;
                float centerA = Wave(position, directionA, _WaveScale, time * _WaveSpeed);
                float centerB = Wave(position, directionB, _WaveScale * 1.85, time * _WaveSpeed * 1.35);
                float xA = Wave(position + float2(epsilon, 0.0), directionA, _WaveScale, time * _WaveSpeed);
                float zA = Wave(position + float2(0.0, epsilon), directionA, _WaveScale, time * _WaveSpeed);
                float xB = Wave(position + float2(epsilon, 0.0), directionB, _WaveScale * 1.85, time * _WaveSpeed * 1.35);
                float zB = Wave(position + float2(0.0, epsilon), directionB, _WaveScale * 1.85, time * _WaveSpeed * 1.35);
                float slopeX = ((xA - centerA) + (xB - centerB) * 0.75) / epsilon;
                float slopeZ = ((zA - centerA) + (zB - centerB) * 0.75) / epsilon;

                float2 rippleDelta = position - _RippleCenter.xz;
                float rippleDistance = length(rippleDelta);
                float rippleAge = max(0.0, _Time.y - _RippleStartTime);
                float rippleRadius = rippleAge * _RippleSpeed;
                float rippleEnvelope = saturate(1.0 - rippleAge / 5.0);
                float rippleBand = exp(-pow((rippleDistance - rippleRadius) / max(0.05, _RippleWidth), 2.0)) *
                                   rippleEnvelope * _RippleStrength;
                float2 rippleDirection = rippleDistance > 0.001
                    ? rippleDelta / rippleDistance
                    : float2(0.0, 0.0);
                slopeX += rippleDirection.x * rippleBand * 1.8;
                slopeZ += rippleDirection.y * rippleBand * 1.8;
                return normalize(baseNormal + half3(slopeX, 0.0h, slopeZ) * _WaveStrength);
            }

            half4 Frag(Varyings input) : SV_Target
            {
                half3 normalWS = WaveNormal(input.positionWS.xz, NormalizeNormalPerPixel(input.normalWS));
                half3 viewDirectionWS = GetWorldSpaceNormalizeViewDir(input.positionWS);
                half fresnel = pow(1.0h - saturate(dot(normalWS, viewDirectionWS)), 2.2h);
                float layerA = Wave(input.positionWS.xz, normalize(float2(1.0, 0.35)), _WaveScale, _Time.y * _WaveSpeed);
                float layerB = Wave(input.positionWS.xz, normalize(float2(-0.25, 1.0)), _WaveScale * 1.85, _Time.y * _WaveSpeed * 1.35);
                half crest = saturate(0.5h + 0.5h * (layerA * 0.62h + layerB * 0.38h));

                Light moon = GetMainLight();
                half3 halfDirection = SafeNormalize(moon.direction + viewDirectionWS);
                half glint = pow(saturate(dot(normalWS, halfDirection)), 24.0h) *
                             moon.distanceAttenuation * _SpecularStrength;
                float2 screenUV = input.screenPos.xy / max(input.screenPos.w, 0.0001);
                float2 reflectionUV = float2(screenUV.x, 1.0 - screenUV.y);
                reflectionUV = saturate(reflectionUV + normalWS.xz * _ReflectionDistortion);
                half3 sceneReflection = SampleSceneColor(reflectionUV);
                half3 reflection = lerp(_ReflectionColor.rgb, sceneReflection, _SceneReflectionStrength);
                reflection *= _ReflectionStrength * (0.35h + fresnel * _FresnelStrength);
                half3 color = lerp(_BaseColor.rgb, reflection, saturate(0.32h + fresnel * 0.72h));
                color += _FoamColor.rgb * crest * (0.08h + fresnel * 0.14h);
                color += _MoonColor.rgb * glint;
                color += _MoonColor.rgb * fresnel * crest * 0.10h;
                color = MixFog(color, ComputeFogFactor(input.positionHCS.z));
                return half4(color, _Alpha);
            }
            ENDHLSL
        }
    }
}
