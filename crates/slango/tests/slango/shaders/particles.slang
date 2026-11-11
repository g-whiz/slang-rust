struct Particle { float3 position; float mass; float2 velocity; };

RWStructuredBuffer<Particle> particles;
uniform float step;
uniform uint count;

[shader("compute")]
[numthreads(64, 1, 1)]
void advance(uint3 id : SV_DispatchThreadID)
{
    if (id.x < count)
    {
        Particle particle = particles[id.x];
        particle.position.xy += particle.velocity * step;
        particle.position.z += particle.mass;
        particle.mass *= 2.0;
        particles[id.x] = particle;
    }
}
