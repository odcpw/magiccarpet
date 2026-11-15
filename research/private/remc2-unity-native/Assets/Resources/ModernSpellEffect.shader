Shader "MagicCarpet/ModernSpellEffect"
{
    Properties
    {
        _Tint ("Source colour", Color) = (1, .35, .035, 1)
        _EmissionStrength ("Emission", Float) = 2.5
        _Impact ("Radial impact", Float) = 0
        _Halo ("Ownership glow", Float) = 0
        _Voxel ("Block particles", Float) = 0
        _EffectSeed ("Occupant variation", Float) = 0
    }
    SubShader
    {
        Tags { "RenderPipeline"="UniversalPipeline" "Queue"="Transparent" "RenderType"="Transparent" }
        Pass
        {
            Name "SourceHotEffect"
            Tags { "LightMode"="UniversalForward" }
            Blend SrcAlpha OneMinusSrcAlpha
            ZWrite Off Cull Off
            HLSLPROGRAM
            #pragma target 3.5
            #pragma vertex Vert
            #pragma fragment Frag
            #pragma multi_compile_fog
            #include "Packages/com.unity.render-pipelines.universal/ShaderLibrary/Core.hlsl"
            CBUFFER_START(UnityPerMaterial)
                half4 _Tint;
                float _EmissionStrength, _Impact, _Halo, _Voxel, _EffectSeed;
            CBUFFER_END
            float4 _ModernWind;
            struct A { float4 p:POSITION; float2 uv:TEXCOORD0; };
            struct V { float4 p:SV_POSITION; float2 uv:TEXCOORD0; half fog:TEXCOORD1; };
            V Vert(A a)
            {
                V o;
                float3 world = TransformObjectToWorld(a.p.xyz);
                float height = 1 - a.uv.y;
                float2 wind = _ModernWind.xy / max(1, length(_ModernWind.xy) / 40);
                world.xz += wind * .008 * height * height * (1 - _Impact) * (1 - _Halo);
                o.p = TransformWorldToHClip(world);
                o.uv = float2(a.uv.x, height);
                o.fog = ComputeFogFactor(o.p.z);
                return o;
            }
            float Hash(float2 p)
            {
                float3 a = frac(float3(p.xyx) * .1031);
                a += dot(a, a.yzx + 33.33);
                return frac((a.x + a.y) * a.z);
            }
            float Noise(float2 p)
            {
                float2 cell = floor(p), f = frac(p);
                f = f*f*(3-2*f);
                return lerp(lerp(Hash(cell), Hash(cell+float2(1,0)), f.x),
                    lerp(Hash(cell+float2(0,1)), Hash(cell+1), f.x), f.y);
            }
            half4 Frag(V i):SV_Target
            {
                float2 uv = lerp(i.uv, (floor(i.uv*32)+.5)/32, _Voxel);
                float2 p = uv*2-1;
                float t = _Time.y;
                float grain = Noise(float2(uv.x*5+_EffectSeed, uv.y*5-t*2));
                float fine = Noise(uv*11+float2(_EffectSeed,-t*3));
                float density = .7*grain+.3*fine;
                float alpha, heat;
                if (_Halo > .5)
                {
                    float radius = length(p);
                    alpha = 1-smoothstep(.05, .98, radius);
                    heat = saturate(1-radius);
                }
                else if (_Impact > .5)
                {
                    float edge = .88+.12*(density-.5)-length(p);
                    alpha = smoothstep(0, .16, edge)*(.72+.28*density);
                    heat = saturate(1-length(p)*.9+(density-.5)*.4);
                }
                else
                {
                    float y = saturate(uv.y/.95);
                    float center = p.x+sin(y*7-t*4+_EffectSeed)*.2*y*y+(density-.5)*.4*y;
                    float width = .18*(1-smoothstep(0,.25,y))+.65*max(0,sin(PI*y))*sqrt(1-y);
                    width *= lerp(.7,1.15,density);
                    alpha = smoothstep(-.04,.06,width-abs(center))*(1-smoothstep(.78,1,y));
                    heat = saturate(1-y*.9-abs(center)*.6+(density-.5)*.4);
                }
                half3 hue = _Tint.rgb / max(.001, max(_Tint.r, max(_Tint.g, _Tint.b)));
                half3 color = lerp(hue*half3(.7,.45,.35), hue*.5+half3(.5,.36,.08), heat);
                color = lerp(color, half3(1,.9,.58), smoothstep(.86,1,heat)*.65);
                if (_Halo > .5)
                    color = hue*(.5+heat)+half3(1,1,1)*pow(heat,6)*.6;
                color *= 1+_EmissionStrength*.3;
                return half4(MixFog(color,i.fog),alpha*_Tint.a);
            }
            ENDHLSL
        }
    }
    FallBack Off
}
