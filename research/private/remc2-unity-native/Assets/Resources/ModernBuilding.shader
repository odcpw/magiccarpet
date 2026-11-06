Shader "MagicCarpet/ModernBuilding"
{
    Properties
    {
        _StoneMap ("Stone albedo and roughness", 2D) = "white" {}
        _WindowLight ("Window light", Range(0,1)) = 0
    }
    SubShader
    {
        Tags { "RenderPipeline"="UniversalPipeline" "RenderType"="Opaque" "Queue"="Geometry" }
        HLSLINCLUDE
        #include "Packages/com.unity.render-pipelines.universal/ShaderLibrary/Core.hlsl"
        CBUFFER_START(UnityPerMaterial)
            float4 _StoneMap_ST;
            float _WindowLight;
        CBUFFER_END
        struct A { float4 positionOS:POSITION; float3 normalOS:NORMAL; float2 uv:TEXCOORD0; float2 material:TEXCOORD1; half4 color:COLOR; UNITY_VERTEX_INPUT_INSTANCE_ID };
        struct V { float4 positionCS:SV_POSITION; float3 world:TEXCOORD0; float3 normal:TEXCOORD1; float3 local:TEXCOORD2; half4 color:COLOR; float material:TEXCOORD3; half fog:TEXCOORD4; UNITY_VERTEX_INPUT_INSTANCE_ID };
        V Vert(A v)
        {
            V o; UNITY_SETUP_INSTANCE_ID(v); UNITY_TRANSFER_INSTANCE_ID(v,o);
            o.world=TransformObjectToWorld(v.positionOS.xyz);o.positionCS=TransformWorldToHClip(o.world);
            o.normal=TransformObjectToWorldNormal(v.normalOS);o.local=v.positionOS.xyz;
            o.color=v.color;o.material=v.material.x;o.fog=ComputeFogFactor(o.positionCS.z);return o;
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
            #pragma multi_compile _ _MAIN_LIGHT_SHADOWS _MAIN_LIGHT_SHADOWS_CASCADE _MAIN_LIGHT_SHADOWS_SCREEN
            #pragma multi_compile_fragment _ _SHADOWS_SOFT _SHADOWS_SOFT_LOW _SHADOWS_SOFT_MEDIUM _SHADOWS_SOFT_HIGH
            #pragma multi_compile_fog
            #include "Packages/com.unity.render-pipelines.universal/ShaderLibrary/Lighting.hlsl"
            #include "ModernLocalLighting.hlsl"
            TEXTURE2D(_StoneMap); SAMPLER(sampler_StoneMap);
            float StoneHash(float2 cell) { return frac(sin(dot(cell,float2(127.1,311.7)))*43758.5453); }
            half4 Frag(V i):SV_Target
            {
                UNITY_SETUP_INSTANCE_ID(i);
                half3 n=normalize(i.normal), view=GetWorldSpaceNormalizeViewDir(i.world);
                float3 weights=pow(abs(n),4);weights/=max(.001,dot(weights,1));
                float3 p=i.local*.36;
                half3 texel= SAMPLE_TEXTURE2D(_StoneMap,sampler_StoneMap,p.yz).rgb*weights.x
                    +SAMPLE_TEXTURE2D(_StoneMap,sampler_StoneMap,p.xz).rgb*weights.y
                    +SAMPLE_TEXTURE2D(_StoneMap,sampler_StoneMap,p.xy).rgb*weights.z;
                half grain=saturate(dot(texel,half3(.3,.5,.2))*1.7);
                int kind=(int)round(i.material);
                half3 albedo=half3(.18,.25,.31)*lerp(.8,1.12,grain);
                half smoothness=.14;
                if(kind==0||kind==1)
                {
                    float2 surface=abs(n.x)>abs(n.z)?i.local.zy:i.local.xy;
                    float row=floor(surface.y/1.05);
                    float2 bond=float2(surface.x/2.15+frac(row*.5),surface.y/1.05);
                    float2 edge=min(frac(bond),1-frac(bond))*float2(2.15,1.05);
                    float distance=min(edge.x,edge.y);
                    float aa=max(fwidth(distance),.008);
                    float mortar=1-smoothstep(.018-aa,.06+aa,distance);
                    float block=StoneHash(floor(bond));
                    half3 upper=half3(.20,.285,.355),lower=half3(.075,.12,.14);
                    albedo=lerp(lower,upper,smoothstep(4.6,5.5,i.local.y));
                    albedo*=lerp(.82,1.14,block)*lerp(.8,1.13,grain);
                    albedo=lerp(albedo,half3(.035,.048,.054),mortar*.65);
                    // A shallow bevel catches light without adding per-brick
                    // geometry. Filter its edge to keep distant masonry stable.
                    float relief=smoothstep(.01,.14+aa,distance)*.045+(grain-.5)*.018;
                    float3 dx=ddx(i.world),dy=ddy(i.world);
                    float3 rx=cross(dy,n),ry=cross(n,dx);float determinant=dot(dx,rx);
                    n=normalize(n-sign(determinant)*(ddx(relief)*rx+ddy(relief)*ry)/max(abs(determinant),.00001));
                }
                else if(kind==2)
                {
                    float2 tile=i.local.xz/float2(.9,1.1);
                    tile.x+=frac(floor(tile.y)*.5);
                    float2 edges=min(frac(tile),1-frac(tile));
                    float seam=1-smoothstep(.015,.055+max(fwidth(tile.x),fwidth(tile.y)),min(edges.x,edges.y));
                    albedo=half3(.13,.105,.075)*lerp(.75,1.2,StoneHash(floor(tile)))*lerp(.85,1.1,grain);
                    albedo*=1-seam*.5;smoothness=.23;
                }
                else if(kind==3)
                {
                    float lines=sin(i.local.y*11+sin(i.local.x*2.1)*1.4)*.5+.5;
                    albedo=half3(.105,.048,.022)*lerp(.7,1.3,lines)*lerp(.8,1.2,grain);smoothness=.22;
                }
                else if(kind==4) { albedo=half3(.07,.065,.052)*lerp(.75,1.2,grain);smoothness=.4; }
                else if(kind==5) { albedo=half3(.085,.055,.023);smoothness=.45; }
                else if(kind==6) { albedo=half3(.028,.021,.016);smoothness=.04; }
                // Subtle ground staining, with texture variation instead of a hard painted band.
                float damp=(1-smoothstep(-.1,1.3,i.local.y))*(.10+.13*grain);
                albedo*=i.color.rgb*(1-damp);
                Light sun=GetMainLight(TransformWorldToShadowCoord(i.world));
                half diffuse=saturate(dot(n,sun.direction));
                half3 ambient=max(SampleSH(n),half3(.025,.027,.03));
                half3 color=albedo*(ambient+sun.color*(diffuse*sun.shadowAttenuation));
                half3 h=normalize(sun.direction+view);
                half spec=pow(saturate(dot(n,h)),lerp(12,110,smoothness))*smoothness*.22;
                color+=sun.color*spec*sun.shadowAttenuation;
                color+=albedo*ModernLocalLight(i.world,n,view,smoothness);
                if(kind==5)
                {
                    float warmth=.55+.35*(1-smoothstep(1.55,3.1,i.local.y));
                    float curtain=.88+.12*sin(i.local.x*9+i.local.z*9);
                    color+=half3(1.25,.86,.42)*_WindowLight*warmth*curtain;
                }
                return half4(MixFog(color,i.fog),1);
            }
            ENDHLSL
        }
        Pass
        {
            Name "ShadowCaster"
            Tags { "LightMode"="ShadowCaster" }
            ZWrite On ZTest LEqual ColorMask 0
            HLSLPROGRAM
            #pragma vertex ShadowVert
            #pragma fragment DepthFrag
            #pragma multi_compile_instancing
            #include "Packages/com.unity.render-pipelines.universal/ShaderLibrary/Shadows.hlsl"
            float3 _LightDirection;
            float4 ShadowVert(A v):SV_POSITION
            {
                UNITY_SETUP_INSTANCE_ID(v);
                float3 p=TransformObjectToWorld(v.positionOS.xyz),n=TransformObjectToWorldNormal(v.normalOS);
                float4 clip=TransformWorldToHClip(ApplyShadowBias(p,n,_LightDirection));
                #if UNITY_REVERSED_Z
                    clip.z=min(clip.z,UNITY_NEAR_CLIP_VALUE);
                #else
                    clip.z=max(clip.z,UNITY_NEAR_CLIP_VALUE);
                #endif
                return clip;
            }
            half4 DepthFrag():SV_Target { return 0; }
            ENDHLSL
        }
        Pass
        {
            Name "DepthOnly"
            Tags { "LightMode"="DepthOnly" }
            ZWrite On ColorMask R
            HLSLPROGRAM
            #pragma vertex Vert
            #pragma fragment DepthFrag
            #pragma multi_compile_instancing
            half4 DepthFrag(V i):SV_Target { return i.positionCS.z; }
            ENDHLSL
        }
    }
}
