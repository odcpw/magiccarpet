Shader "MagicCarpet/Enhanced/Water"
{
    Properties
    {
        [MainColor] _BaseColor ("Base Color", Color) = (0.008, 0.04, 0.12, 1)
        _ReflectionColor ("Sky Reflection", Color) = (0.025, 0.15, 0.42, 1)
        _FoamColor ("Wave Crest", Color) = (0.08, 0.25, 0.60, 1)
        _WaveScale ("Wave Scale", Float) = 0.055
        _WaveSpeed ("Wave Speed", Float) = 0.22
        _WaveStrength ("Wave Normal Strength", Range(0, 1)) = 0.10
        _FresnelStrength ("Fresnel Strength", Range(0, 2)) = 0.55
        _SpecularStrength ("Moon Glint Strength", Range(0, 4)) = 0.8
        _Alpha ("Surface Opacity", Range(0, 1)) = 0.84
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
            #include "Packages/com.unity.render-pipelines.universal/ShaderLibrary/Lighting.hlsl"

            CBUFFER_START(UnityPerMaterial)
                half4 _BaseColor;
                half4 _ReflectionColor;
                half4 _FoamColor;
                half _WaveScale;
                half _WaveSpeed;
                half _WaveStrength;
                half _FresnelStrength;
                half _SpecularStrength;
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
            };

            Varyings Vert(Attributes input)
            {
                Varyings output;
                VertexPositionInputs positionInputs = GetVertexPositionInputs(input.positionOS.xyz);
                VertexNormalInputs normalInputs = GetVertexNormalInputs(input.normalOS);
                output.positionHCS = positionInputs.positionCS;
                output.positionWS = positionInputs.positionWS;
                output.normalWS = normalInputs.normalWS;
                return output;
            }

            float Wave(float2 position, float2 direction, float time)
            {
                return sin(dot(position, direction) + time);
            }

            half3 WaveNormal(float2 position, half3 baseNormal)
            {
                float time = _Time.y * _WaveSpeed;
                float2 scaled = position * _WaveScale;
                float waveX = Wave(scaled + float2(0.035, 0.0), float2(1.0, 0.35), time);
                float waveZ = Wave(scaled + float2(0.0, 0.035), float2(-0.25, 1.0), time * 0.82);
                float center = Wave(scaled, float2(1.0, 0.35), time);
                float slopeX = center - waveX;
                float slopeZ = center - waveZ;
                return normalize(baseNormal + half3(slopeX, 0.0h, slopeZ) * _WaveStrength);
            }

            half4 Frag(Varyings input) : SV_Target
            {
                half3 normalWS = WaveNormal(input.positionWS.xz, NormalizeNormalPerPixel(input.normalWS));
                half3 viewDirectionWS = GetWorldSpaceNormalizeViewDir(input.positionWS);
                half fresnel = pow(1.0h - saturate(dot(normalWS, viewDirectionWS)), 3.0h);
                half crest = saturate(
                    Wave(input.positionWS.xz * _WaveScale, float2(1.0, 0.35), _Time.y * _WaveSpeed) * 0.5h +
                    Wave(input.positionWS.xz * (_WaveScale * 1.7h), float2(-0.25, 1.0), _Time.y * _WaveSpeed * 0.82) * 0.5h);

                Light moon = GetMainLight();
                half3 halfDirection = SafeNormalize(moon.direction + viewDirectionWS);
                half glint = pow(saturate(dot(normalWS, halfDirection)), 72.0h) *
                             moon.distanceAttenuation * _SpecularStrength;
                half3 reflection = _ReflectionColor.rgb * (0.25h + fresnel * _FresnelStrength);
                half3 color = lerp(_BaseColor.rgb, reflection, saturate(0.20h + fresnel * 0.50h));
                color += _FoamColor.rgb * crest * 0.045h;
                color += _ReflectionColor.rgb * glint;
                color = MixFog(color, ComputeFogFactor(input.positionHCS.z));
                return half4(color, _Alpha);
            }
            ENDHLSL
        }
    }
}
