// The indexed pixels and palette are loaded from the validated external owner
// cache at runtime. No owner image data is embedded in this shader.
Shader "MagicCarpet/Enhanced/OwnerEntitySprite"
{
    Properties
    {
        _Indices ("Owner indexed pixels", 2D) = "black" {}
        _Palette ("Owner PALN palette", 2D) = "white" {}
        _Tint ("Enhanced night tint", Color) = (0.92, 0.96, 1, 1)
        _EmissionStrength ("Owner pixel lift", Range(0, 1)) = 0.06
        [HDR] _OutlineColor ("Moonlit outline color", Color) = (0.20, 0.42, 0.95, 1)
        _OutlineStrength ("Moonlit outline strength", Range(0, 1)) = 0.72
    }

    SubShader
    {
        Tags
        {
            "Queue" = "AlphaTest"
            "RenderType" = "TransparentCutout"
            "RenderPipeline" = "UniversalPipeline"
        }

        Cull Off
        ZWrite On
        ZTest LEqual

        Pass
        {
            Name "OwnerIndexedActor"
            Tags { "LightMode" = "UniversalForward" }

            HLSLPROGRAM
            #pragma vertex Vert
            #pragma fragment Frag
            #pragma multi_compile_fog

            #include "Packages/com.unity.render-pipelines.universal/ShaderLibrary/Core.hlsl"

            TEXTURE2D(_Indices);
            SAMPLER(sampler_Indices);
            TEXTURE2D(_Palette);
            SAMPLER(sampler_Palette);
            float4 _Indices_TexelSize;

            CBUFFER_START(UnityPerMaterial)
                half4 _Tint;
                half _EmissionStrength;
                half4 _OutlineColor;
                half _OutlineStrength;
            CBUFFER_END

            #define ENHANCED_WARM_LIGHT_CAP 4
            float4 _EnhancedWarmLightPositions[ENHANCED_WARM_LIGHT_CAP];
            float4 _EnhancedWarmLightColors[ENHANCED_WARM_LIGHT_CAP];
            float4 _EnhancedWarmLightRanges[ENHANCED_WARM_LIGHT_CAP];
            int _EnhancedWarmLightCount;

            struct Attributes
            {
                float4 positionOS : POSITION;
                float2 uv : TEXCOORD0;
            };

            struct Varyings
            {
                float4 positionHCS : SV_POSITION;
                float3 positionWS : TEXCOORD0;
                float2 uv : TEXCOORD1;
                half fogFactor : TEXCOORD2;
            };

            Varyings Vert(Attributes input)
            {
                Varyings output;
                VertexPositionInputs positionInputs =
                    GetVertexPositionInputs(input.positionOS.xyz);
                output.positionHCS = positionInputs.positionCS;
                output.positionWS = positionInputs.positionWS;
                output.uv = input.uv;
                output.fogFactor = ComputeFogFactor(positionInputs.positionCS.z);
                return output;
            }

            half3 EvaluateWarmLight(float3 positionWS)
            {
                half3 contribution = 0.0h;
                int count = min(_EnhancedWarmLightCount, ENHANCED_WARM_LIGHT_CAP);
                for (int index = 0; index < count; index++)
                {
                    float3 delta = _EnhancedWarmLightPositions[index].xyz - positionWS;
                    float distanceToLight = length(delta);
                    float range = max(_EnhancedWarmLightRanges[index].x, 0.001);
                    half attenuation = saturate(1.0h - distanceToLight / range);
                    attenuation *= attenuation;
                    half3 lightColor = _EnhancedWarmLightColors[index].rgb;
                    half intensity = _EnhancedWarmLightColors[index].a;
                    contribution += lightColor * intensity * attenuation * 0.22h;
                }
                return contribution;
            }

            half4 Frag(Varyings input) : SV_Target
            {
                half sampleValue = SAMPLE_TEXTURE2D(
                    _Indices,
                    sampler_Indices,
                    input.uv).r;
                half paletteIndex = floor(sampleValue * 255.0h + 0.5h);
                if (paletteIndex < 0.5h && _OutlineStrength > 0.001h)
                {
                    float2 texel = _Indices_TexelSize.xy;
                    half right = SAMPLE_TEXTURE2D(
                        _Indices,
                        sampler_Indices,
                        input.uv + float2(texel.x, 0.0)).r;
                    half left = SAMPLE_TEXTURE2D(
                        _Indices,
                        sampler_Indices,
                        input.uv - float2(texel.x, 0.0)).r;
                    half up = SAMPLE_TEXTURE2D(
                        _Indices,
                        sampler_Indices,
                        input.uv + float2(0.0, texel.y)).r;
                    half down = SAMPLE_TEXTURE2D(
                        _Indices,
                        sampler_Indices,
                        input.uv - float2(0.0, texel.y)).r;
                    half neighbor = max(
                        max(step(0.5h, right * 255.0h), step(0.5h, left * 255.0h)),
                        max(step(0.5h, up * 255.0h), step(0.5h, down * 255.0h)));
                    if (neighbor > 0.5h)
                    {
                        half3 outline = _OutlineColor.rgb * _OutlineStrength;
                        outline = MixFog(outline, input.fogFactor);
                        return half4(outline, 1.0h);
                    }
                }
                clip(paletteIndex - 0.5h);

                half3 color = SAMPLE_TEXTURE2D(
                    _Palette,
                    sampler_Palette,
                    float2((paletteIndex + 0.5h) / 256.0h, 0.5h)).rgb;
                // Preserve the external owner palette while lifting its night
                // fill enough for small creatures to separate from the cool
                // terrain. This is display lighting, not a source-pixel edit.
                half3 moonFill = half3(0.88h, 0.94h, 1.0h);
                half3 warm = EvaluateWarmLight(input.positionWS);
                color *= _Tint.rgb * (moonFill + warm);
                color += color * _EmissionStrength;
                color = MixFog(color, input.fogFactor);
                return half4(color, 1.0h);
            }
            ENDHLSL
        }
    }
    FallBack Off
}
