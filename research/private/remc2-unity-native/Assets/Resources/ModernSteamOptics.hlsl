#ifndef MODERN_STEAM_OPTICS_INCLUDED
#define MODERN_STEAM_OPTICS_INCLUDED
TEXTURE3D(_ModernDetailNoise); SAMPLER(sampler_ModernDetailNoise);
float _ModernDensityDetail;
float4 _ModernWindOffset;

// Visible steam and flame extinction must use the same advected optical shape.
// This detail is independent of the conserved simulation/wake density field.
void ModernSteamOptics(float3 p, float clock, out float3 samplePosition,
    out float multiplier, out float detail)
{
    samplePosition=p; multiplier=1; detail=.5;
    if(_ModernDensityDetail<.5) return;
    float3 q=p-float3(_ModernWindOffset.x,clock*.65,_ModernWindOffset.y);
    float coarse=SAMPLE_TEXTURE3D_LOD(_ModernDetailNoise,sampler_ModernDetailNoise,(q*.55+17.5)/128,0).r;
    float fine=SAMPLE_TEXTURE3D_LOD(_ModernDetailNoise,sampler_ModernDetailNoise,(q*1.2-11.5)/128,0).r;
    // A thick front saturates regardless of density multiplication. Distorting
    // its 3D sampling domain produces advecting lobes rather than a solid plug.
    samplePosition+=float3(coarse-.5,fine-.5,(coarse-fine)*.5)*3;
    detail=smoothstep(.28,.72,coarse*.65+fine*.35);
    multiplier=lerp(.15,1.85,detail);
}

float ModernSteamBoundary(float3 p, float3 origin, float size)
{
    float3 edge=min(p-origin,origin+size-p);
    return smoothstep(0,.8,min(edge.x,min(edge.y,edge.z)));
}
#endif
