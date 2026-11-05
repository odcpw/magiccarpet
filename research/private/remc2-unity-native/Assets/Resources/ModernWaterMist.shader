Shader "MagicCarpet/ModernWaterMist"
{
    Properties { _Pattern("Pattern",2D)="white" {} _Mist("Condensation",Float)=0 _Glow("Glow",Float)=0 }
    SubShader
    {
        Tags { "RenderPipeline"="UniversalPipeline" "Queue"="Transparent+10" "RenderType"="Transparent" }
        Pass
        {
            Blend SrcAlpha OneMinusSrcAlpha
            ZWrite Off Cull Off
            HLSLPROGRAM
            #pragma target 4.5
            #pragma vertex Vert
            #pragma fragment Frag
            #pragma multi_compile_fog
            #include "Packages/com.unity.render-pipelines.universal/ShaderLibrary/Core.hlsl"
            #include "Packages/com.unity.render-pipelines.universal/ShaderLibrary/DeclareDepthTexture.hlsl"
            TEXTURE2D(_Pattern); SAMPLER(sampler_Pattern);
            TEXTURE2D(_WaterDisplacement); SAMPLER(sampler_WaterDisplacement);
            TEXTURE2D(_WaterCoast); SAMPLER(sampler_WaterCoast);
            TEXTURE2D(_WaterLocal); SAMPLER(sampler_WaterLocal);
            float4 _WaterLocalRect, _ModernWind;
            float _WaterTime;
            CBUFFER_START(UnityPerMaterial)
            float _Mist, _Glow;
            CBUFFER_END
            struct A { float4 positionOS:POSITION; float2 uv:TEXCOORD0; half4 color:COLOR; };
            struct V { float4 positionCS:SV_POSITION; float2 uv:TEXCOORD0; half4 color:COLOR; float3 world:TEXCOORD1; half fog:TEXCOORD2; };
            V Vert(A i) { V o; o.world=TransformObjectToWorld(i.positionOS.xyz); o.positionCS=TransformWorldToHClip(o.world); o.uv=i.uv; o.color=i.color; o.fog=ComputeFogFactor(o.positionCS.z); return o; }
            half4 Frag(V i):SV_Target
            {
                float2 q=i.uv*2-1;
                float density=SAMPLE_TEXTURE2D(_Pattern,sampler_Pattern,i.uv*1.6+0.17).b;
                float shape=exp(-dot(q,q)*4)*(1-smoothstep(.65,1,length(q)));
                shape*=lerp(1,smoothstep(.18,.7,density),_Mist);
                float scene=LinearEyeDepth(SampleSceneDepth(GetNormalizedScreenSpaceUV(i.positionCS)),_ZBufferParams);
                float eye=-TransformWorldToView(i.world).z;
                float soft=saturate((scene-eye)*2);
                float coast=SAMPLE_TEXTURE2D(_WaterCoast,sampler_WaterCoast,i.world.xz/1024).r;
                float2 displacement=SAMPLE_TEXTURE2D(_WaterDisplacement,sampler_WaterDisplacement,i.world.xz/256+0.5/256).rb;
                float2 rest=i.world.xz-displacement*smoothstep(0,12,coast);
                float4 coastData=SAMPLE_TEXTURE2D(_WaterCoast,sampler_WaterCoast,rest/1024);
                coast=coastData.r;
                float wave=SAMPLE_TEXTURE2D(_WaterDisplacement,sampler_WaterDisplacement,rest/256+0.5/256).g*smoothstep(0,12,coast);
                float2 luv=(rest-_WaterLocalRect.xy)*_WaterLocalRect.w;
                float edge=min(min(luv.x,luv.y),min(1-luv.x,1-luv.y));
                wave+=SAMPLE_TEXTURE2D(_WaterLocal,sampler_WaterLocal,saturate(luv)).r*smoothstep(0,.06,edge)*smoothstep(0,2,coast);
                float exposure=saturate(-dot(normalize(_ModernWind.xy+.001),coastData.ba));
                wave+=sin(max(0,coast)*.8+_WaterTime*1.25)*exp(-max(0,coast)*.18)
                    *smoothstep(0,2,coast)*.15*saturate(_ModernWind.z)*(.2+.8*exposure);
                soft*=lerp(1,smoothstep(-.08,.3,i.world.y-wave),step(0,coast));
                half3 color=MixFog(i.color.rgb*lerp(1,5,_Glow),i.fog);
                return half4(color,shape*soft*i.color.a*lerp(1,2,_Glow));
            }
            ENDHLSL
        }
    }
}
