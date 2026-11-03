Shader "MagicCarpet/ModernSmoke"
{
    Properties
    {
        _Tint ("Smoke tint", Color) = (0.48, 0.50, 0.52, 1)
        _Opacity ("Opacity", Range(0,1)) = 0.35
        _Seed ("Variation seed", Float) = 0
        _Softness ("Intersection fade distance", Float) = 2
    }
    SubShader
    {
        Tags { "RenderPipeline"="UniversalPipeline" "RenderType"="Transparent" "Queue"="Transparent" "IgnoreProjector"="True" }
        Cull Off
        ZWrite Off
        ZTest LEqual
        Blend SrcAlpha OneMinusSrcAlpha
        Pass
        {
            Name "SmokeForward"
            Tags { "LightMode"="UniversalForward" }
            HLSLPROGRAM
            #pragma target 3.5
            #pragma vertex Vert
            #pragma fragment Frag
            #pragma multi_compile_fog
            #define _SURFACE_TYPE_TRANSPARENT 1
            #include "Packages/com.unity.render-pipelines.universal/ShaderLibrary/Core.hlsl"
            #include "Packages/com.unity.render-pipelines.universal/ShaderLibrary/Lighting.hlsl"
            #include "Packages/com.unity.render-pipelines.universal/ShaderLibrary/DeclareDepthTexture.hlsl"
            CBUFFER_START(UnityPerMaterial)
                half4 _Tint;
                float _Opacity, _Seed, _Softness;
            CBUFFER_END
            // Global diagnostic switch, zero in ordinary presentation.
            float _ModernSmokeDebug;
            struct Attributes { float4 positionOS:POSITION; float2 uv:TEXCOORD0; };
            struct Varyings { float4 positionCS:SV_POSITION; float3 positionWS:TEXCOORD0; float2 uv:TEXCOORD1; half fog:TEXCOORD2; float3 particle:TEXCOORD3; };
            Varyings Vert(Attributes v)
            {
                Varyings o;
                VertexPositionInputs p=GetVertexPositionInputs(v.positionOS.xyz);
                o.positionCS=p.positionCS; o.positionWS=p.positionWS; o.uv=v.uv;
                float3 origin=TransformObjectToWorld(float3(0,0,0));
                // The map repeats every 1024 units, so these phases remain
                // continuous when a particle is shown in an adjacent copy.
                float2 phase=origin.xz*0.00613592315;
                o.particle.xy=float2(sin(phase.x)+cos(phase.y),cos(phase.x)+sin(phase.y))*8;
                o.particle.z=length(TransformObjectToWorldDir(float3(0,1,0),false));
                o.fog=ComputeFogFactor(p.positionCS.z); return o;
            }
            // Integer lattice hashing gives each shared cell corner exactly one
            // value. A sine hash can disagree across neighboring cells after
            // floating-point reassociation and expose visible square seams.
            float Hash(int2 p)
            {
                uint2 bits=asuint(p);
                uint h=bits.x*0x8da6b343u ^ bits.y*0xd8163841u ^ 0xcb1ab31fu;
                h^=h>>16; h*=0x7feb352du; h^=h>>15; h*=0x846ca68bu; h^=h>>16;
                return (h & 0x00ffffffu)*(1.0/16777216.0);
            }
            float Noise(float2 p)
            {
                int2 i=(int2)floor(p); float2 f=frac(p); f=f*f*(3-2*f);
                return lerp(lerp(Hash(i),Hash(i+int2(1,0)),f.x),
                    lerp(Hash(i+int2(0,1)),Hash(i+int2(1,1)),f.x),f.y);
            }
            half4 Frag(Varyings i):SV_Target
            {
                float2 p=(i.uv-0.5)*2;
                float t=_Time.y;
                float2 drift=float2(_Seed*0.731+t*0.025,_Seed*0.417-t*0.035)+i.particle.xy;
                float2 warp=float2(Noise(i.uv*2.6+drift),Noise(i.uv*2.6+drift+17.3))-0.5;
                float2 q=i.uv*3.6+drift+warp*0.38;
                float cloud=Noise(q)*0.58+Noise(q*2.04+9.2)*0.28+Noise(q*4.07-4.3)*0.14;
                // Fade continuously across the whole disc, including a zero
                // first derivative at the billboard boundary.
                float edge=saturate(1-dot(p,p)); edge*=edge;
                float density=edge*smoothstep(0.23,0.70,cloud);
                if (_ModernSmokeDebug > 0.5 && _ModernSmokeDebug < 1.5)
                    return half4(i.uv,0,1);
                if (_ModernSmokeDebug > 1.5 && _ModernSmokeDebug < 2.5)
                    return half4(edge,edge,edge,1);
                if (_ModernSmokeDebug > 2.5 && _ModernSmokeDebug < 3.5)
                    return half4(1,0,1,edge*0.5);
                if (_ModernSmokeDebug > 3.5)
                    return half4(_Tint.rgb,density*_Opacity*0.65);
                float2 screenUV=GetNormalizedScreenSpaceUV(i.positionCS);
                float sceneDepth=LinearEyeDepth(SampleSceneDepth(screenUV),_ZBufferParams);
                float particleDepth=-TransformWorldToView(i.positionWS).z;
                float intersection=saturate((sceneDepth-particleDepth)/max(_Softness,0.001));
                // A puff intersecting the eye/near plane must not become an
                // enormous clipped sheet; fade in over a few of its heights.
                float nearFade=smoothstep(i.particle.z*0.75,max(i.particle.z*2.5,0.001),particleDepth);
                half alpha=saturate(density*intersection*nearFade*_Opacity*_Tint.a*0.65);
                Light moon=GetMainLight();
                half3 lightTint=lerp(half3(1,1,1),moon.color,0.18);
                half3 color=_Tint.rgb*lightTint*lerp(0.76,1.08,cloud);
                return half4(MixFog(color,i.fog),alpha);
            }
            ENDHLSL
        }
    }
    FallBack Off
}
