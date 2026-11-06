Shader "MagicCarpet/ModernTorchFlame"
{
    Properties
    {
        _FlameSeed ("Stable fixture variation", Float) = 0
    }
    SubShader
    {
        Tags { "RenderPipeline"="UniversalPipeline" "Queue"="Transparent+20" "RenderType"="Transparent" }
        Pass
        {
            Name "TorchFlame"
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
                float _FlameSeed;
            CBUFFER_END
            float4 _ModernWind;
            struct A { float4 positionOS:POSITION; float3 normalOS:NORMAL; float2 uv:TEXCOORD0; float4 color:COLOR; };
            struct V { float4 positionCS:SV_POSITION; float2 uv:TEXCOORD0; float3 world:TEXCOORD1; half3 normal:TEXCOORD2; half ember:TEXCOORD3; half fog:TEXCOORD4; };
            V Vert(A i)
            {
                V o;
                float3 world=TransformObjectToWorld(i.positionOS.xyz);
                float height=i.uv.y*(1-i.color.r);
                float t=_Time.y;
                float flutter=sin(t*3.7+_FlameSeed+height*4)*.035+sin(t*6.1+_FlameSeed*1.7-height*3)*.018;
                float2 wind=_ModernWind.xy*min(1,40/max(length(_ModernWind.xy),.001));
                world.xz+=wind*.018*height*height+float2(flutter,flutter*.6)*height*height;
                world.y+=sin(t*4.2+_FlameSeed)*.035*height*height;
                o.world=world; o.positionCS=TransformWorldToHClip(world); o.uv=i.uv;
                o.normal=TransformObjectToWorldNormal(i.normalOS); o.ember=i.color.r;
                o.fog=ComputeFogFactor(o.positionCS.z); return o;
            }
            float Hash(float2 p)
            {
                float3 v=frac(float3(p.xyx)*.1031);
                v+=dot(v,v.yzx+33.33);
                return frac((v.x+v.y)*v.z);
            }
            float Noise(float2 p)
            {
                float2 cell=floor(p), f=frac(p); f=f*f*(3-2*f);
                return lerp(lerp(Hash(cell),Hash(cell+float2(1,0)),f.x),
                    lerp(Hash(cell+float2(0,1)),Hash(cell+1),f.x),f.y);
            }
            half4 Frag(V i):SV_Target
            {
                float2 p=i.uv*2-1;
                float t=_Time.y;
                float grain=Noise(float2(i.uv.x*4+_FlameSeed,i.uv.y*4-t*1.8));
                float fine=Noise(float2(i.uv.x*9-_FlameSeed*.7,i.uv.y*9-t*2.9));
                float density=grain*.7+fine*.3;
                float height=.80+.10*sin(t*2.3+_FlameSeed)+.05*sin(t*5.1+_FlameSeed*1.9);
                float y=saturate(i.uv.y/height);
                float center=p.x+sin(y*7-t*3.2+_FlameSeed)*.14*y*y+(density-.5)*.38*y;
                // A rounded lower body and thin, moving tip, rather than a straight
                // triangular taper. Advected density breaks up the rising tongue.
                float width=.60*sqrt(max(0,sin(PI*sqrt(y))));
                width*=lerp(.65,1.15,density);
                float edgeDistance=width-abs(center);
                float feather=max(.055,fwidth(edgeDistance)*1.25);
                float body=smoothstep(-feather,feather*2,edgeDistance);
                float vertical=smoothstep(.01,.09,y)*(1-smoothstep(.83,1,y));
                float opacity=body*vertical*(.35+.55*smoothstep(.15,.8,density));

                // Keep most of the flame in a restrained orange range. Only a small
                // pocket above the fuel reaches white-hot HDR, preserving shape under
                // ACES/bloom instead of clipping the entire card to neon red/yellow.
                float core=exp(-center*center*70-(y-.19)*(y-.19)*90)*smoothstep(.2,.65,density);
                half3 fire=lerp(half3(.48,.055,.009),half3(1.65,.45,.075),density);
                fire*=lerp(.25,1,smoothstep(.025,.16,y));
                fire=lerp(fire,half3(4.2,3.0,1.45),core);

                // The mesh has three planes separated by 60 degrees. Normalize their
                // facing weights so overlapping sheets do not multiply brightness;
                // the strongest-facing sheet carries most detail, edge-on ones fade.
                float2 normal=normalize(i.normal.xz+float2(.000001,0));
                float2 view=GetWorldSpaceNormalizeViewDir(i.world).xz;
                float2 next=float2(normal.x*.5-normal.y*.8660254,normal.x*.8660254+normal.y*.5);
                float2 last=float2(-normal.x*.5-normal.y*.8660254,normal.x*.8660254-normal.y*.5);
                float3 facing=abs(float3(dot(normal,view),dot(next,view),dot(last,view)));
                facing*=facing; facing*=facing;
                float weight=facing.x/max(dot(facing,1),.00001);
                float alpha=opacity*weight;
                float emberShape=1-smoothstep(.75,1,length(p));
                half3 ember=lerp(half3(.18,.008,.001),half3(1.4,.14,.008),density);
                fire=lerp(fire,ember,i.ember);
                alpha=lerp(alpha,emberShape,i.ember);
                clip(alpha-.001);
                return half4(MixFog(fire,i.fog),alpha);
            }
            ENDHLSL
        }
    }
    FallBack Off
}
