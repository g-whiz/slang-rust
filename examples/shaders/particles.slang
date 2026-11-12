struct Particle
{
    float3 position;
    float mass;
    float2 velocity;
    uint steps;
};

RWStructuredBuffer<Particle> particles;
RWStructuredBuffer<float> energy;
uniform float2 gravity;
uniform float dt;
uniform uint count;

[shader("compute")]
[numthreads(64, 1, 1)]
void advance(uint3 id : SV_DispatchThreadID)
{
    if (id.x >= count)
    {
        return;
    }
    Particle particle = particles[id.x];
    particle.velocity += gravity * dt;
    particle.position.xy += particle.velocity * dt;
    particle.steps += 1;
    particles[id.x] = particle;
    energy[id.x] = 0.5 * particle.mass * dot(particle.velocity, particle.velocity);
}
