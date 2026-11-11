struct Particle
{
    float3 position;
    float mass;
    float2 velocity;
};

RWStructuredBuffer<Particle> particles;

[shader("compute")]
[numthreads(64, 1, 1)]
void step(uint3 id : SV_DispatchThreadID)
{
    particles[id.x].position.xy += particles[id.x].velocity;
}
