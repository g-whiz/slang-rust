struct Motion
{
    float2 velocity;
    float drag;
};

struct Particle
{
    float3 position;
    float mass;
    Motion motion;
    uint id;
};

struct Wind
{
    float2 force;
};

struct Step
{
    float dt;
    Wind wind;
};

RWStructuredBuffer<Particle> particles;
uniform Step step;
uniform Motion boost;
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
    particle.motion.velocity += (step.wind.force + boost.velocity) * step.dt;
    particle.motion.velocity *= 1.0 - particle.motion.drag * boost.drag;
    particle.position.xy += particle.motion.velocity * step.dt;
    particle.position.z += particle.mass;
    particle.mass *= 2.0;
    particle.id = particle.id * 3 + id.x;
    particles[id.x] = particle;
}
