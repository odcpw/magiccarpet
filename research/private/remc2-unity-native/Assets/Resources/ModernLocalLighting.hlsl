#ifndef MODERN_LOCAL_LIGHTING_INCLUDED
#define MODERN_LOCAL_LIGHTING_INCLUDED
// Eight nearest/highest influence cosmetic emitters. No shadow maps or extra passes.
// Soft inverse-square falloff with finite support; intensities are art-calibrated,
// not a conversion of MC2's indexed palette brightness into photometric units.
int _ModernLightCount;
float4 _ModernLightPosition[8];
float4 _ModernLightColor[8];
float4 _ModernLightDirection[8];
half ModernEmitterMask(int index,float3 p)
{
    // Window emitters sit 0.8 units outside their glazing. Include the facade
    // in the rear allowance, while blocking the opposite side of the building.
    // Torches keep their omnidirectional field.
    float4 direction=_ModernLightDirection[index];
    float outward=dot(p-_ModernLightPosition[index].xyz,direction.xyz);
    return lerp(1,smoothstep(-.95,-.75,outward),direction.w);
}
half3 ModernLocalLight(float3 p, half3 normal, half3 view, half smoothness)
{
    half3 result=0;
    [loop] for (int k=0; k<_ModernLightCount; ++k)
    {
        float3 delta=_ModernLightPosition[k].xyz-p;
        float d2=dot(delta,delta);
        float range=_ModernLightPosition[k].w;
        float edge=saturate(1-d2/(range*range));
        if (edge <= 0) continue;
        float3 direction=delta*rsqrt(max(d2,.01));
        float attenuation=edge*edge/(1+d2*.16)*ModernEmitterMask(k,p);
        half diffuse=saturate(dot(normal,direction));
        half3 halfway=normalize(direction+view+float3(0,.0001,0));
        half spec=pow(saturate(dot(normal,halfway)),lerp(8,100,smoothness))*smoothness;
        result+=_ModernLightColor[k].rgb*attenuation*(diffuse*.6+spec*.65);
    }
    return result;
}
#endif
