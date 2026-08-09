Shader "MagicCarpet/Enhanced/WaterRipple"
{
    Properties
    {
        [HDR] _RippleColor ("Ripple Color", Color) = (0.28, 0.68, 1, 1)
        _RippleStrength ("Ripple Strength", Range(0, 2)) = 0
        _RippleStartTime ("Ripple Start Time", Float) = 0
        _RippleSpeed ("Ripple Speed", Float) = 2.8
        _RippleWidth ("Ripple Width", Float) = 0.12
        _RippleMaxRadius ("Ripple Max Radius", Float) = 4
        _RippleLifetime ("Ripple Lifetime", Float) = 5
        _RippleCenter ("Ripple Center", Vector) = (0, 0, 0, 0)
    }

    SubShader
    {
        Tags
        {
            "Queue" = "Transparent+15"
            "RenderType" = "Transparent"
            "RenderPipeline" = "UniversalPipeline"
        }

        Blend SrcAlpha One
        ZWrite Off
        Cull Off

        Pass
        {
            Name "WaterRipple"
            Tags { "LightMode" = "UniversalForward" }

            HLSLPROGRAM
            #pragma vertex Vert
            #pragma fragment Frag

            #include "Packages/com.unity.render-pipelines.universal/ShaderLibrary/Core.hlsl"

            CBUFFER_START(UnityPerMaterial)
                half4 _RippleColor;
                half _RippleStrength;
                half _RippleStartTime;
                half _RippleSpeed;
                half _RippleWidth;
                half _RippleMaxRadius;
                half _RippleLifetime;
                float4 _RippleCenter;
            CBUFFER_END

            struct Attributes
            {
                float4 positionOS : POSITION;
            };

            struct Varyings
            {
                float4 positionHCS : SV_POSITION;
                float3 positionWS : TEXCOORD0;
            };

            Varyings Vert(Attributes input)
            {
                Varyings output;
                VertexPositionInputs positionInputs = GetVertexPositionInputs(input.positionOS.xyz);
                output.positionHCS = positionInputs.positionCS;
                output.positionWS = positionInputs.positionWS;
                return output;
            }

            half4 Frag(Varyings input) : SV_Target
            {
                float age = max(0.0, _Time.y - _RippleStartTime);
                float radius = age * _RippleSpeed;
                float2 delta = input.positionWS.xz - _RippleCenter.xz;
                float distanceToCenter = length(delta);
                float maxRadius = max(_RippleMaxRadius, 0.01);
                float width = max(_RippleWidth, 0.025);
                float envelope = saturate(1.0 - age / max(_RippleLifetime, 0.1));
                float radialFade = 1.0 - smoothstep(
                    maxRadius * 0.82,
                    maxRadius,
                    distanceToCenter);

                float primary = exp(-pow((distanceToCenter - radius) / width, 2.0));
                float secondaryRadius = max(0.0, radius - max(width * 2.2, 0.18));
                float secondary = exp(-pow(
                    (distanceToCenter - secondaryRadius) / (width * 0.72),
                    2.0)) * 0.42;
                float shimmer = 0.88 + 0.12 * sin(
                    _Time.y * 4.0 + dot(input.positionWS.xz, float2(0.71, -0.43)));
                half intensity = _RippleStrength * envelope * radialFade * shimmer *
                                 (primary + secondary);
                half alpha = saturate(intensity * 0.72h);
                return half4(_RippleColor.rgb * intensity, alpha);
            }
            ENDHLSL
        }
    }
}
