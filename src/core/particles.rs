use crate::prelude::*;
use rand::Rng;

use super::texture::Texture;

pub const TOTAL_MAX_PARTICLES: usize = 10000;

pub struct Particle {
    pub position: Vec3,
    pub velocity: Vec3,
    pub lifetime: f32,
    pub color: Vec4,
}

#[derive(Component)]
pub struct ParticleEmitter {
    pub particles: Vec<Particle>,
    pub origin: Vec3,
    pub spawn_rate: f32,
    pub spawn_timer: f32,
    pub max_particles: usize,
    pub particle_lifetime: f32,
    pub particle_lifetime_randomness: f32,
    pub particle_velocity: Vec3,
    pub particle_velocity_randomness: Vec3,

    pub particle_texture: Option<Texture>,
}

impl Default for ParticleEmitter {
    fn default() -> Self {
        Self {
            particles: Vec::new(),
            origin: Vec3::ZERO,
            spawn_rate: 10.0,
            spawn_timer: 0.0,
            max_particles: 100,
            particle_lifetime: 1.0,
            particle_lifetime_randomness: 0.1,
            particle_velocity: Vec3::new(0.0, 10.0, 0.0),
            particle_velocity_randomness: Vec3::new(0.01, 0.5, 0.01),
            particle_texture: None,
        }
    }
}

impl ParticleEmitter {
    pub fn update(&mut self, dt: f32) {
        self.spawn_timer += dt;

        while self.spawn_timer > 1.0 / self.spawn_rate {
            self.spawn_timer -= 1.0 / self.spawn_rate;

            if self.particles.len() < self.max_particles {
                let mut rng = rand::thread_rng();

                let lifetime = self.particle_lifetime
                    + rng.gen_range(
                        -self.particle_lifetime_randomness..self.particle_lifetime_randomness,
                    );

                let velocity = self.particle_velocity
                    + Vec3::new(
                        rng.gen_range(
                            -self.particle_velocity_randomness.x
                                ..self.particle_velocity_randomness.x,
                        ),
                        rng.gen_range(
                            -self.particle_velocity_randomness.y
                                ..self.particle_velocity_randomness.y,
                        ),
                        rng.gen_range(
                            -self.particle_velocity_randomness.z
                                ..self.particle_velocity_randomness.z,
                        ),
                    );

                let color = Vec4::new(1.0, 1.0, 1.0, 1.0);

                self.particles.push(Particle {
                    position: self.origin,
                    velocity,
                    lifetime,
                    color,
                });
            }
        }

        for particle in self.particles.iter_mut() {
            particle.position += particle.velocity * dt;
            particle.lifetime -= dt;

            particle.color.w = particle.lifetime / self.particle_lifetime;
            particle.color.w = particle.color.w.clamp(0.0, 1.0);
        }

        self.particles.retain(|particle| particle.lifetime > 0.0);
    }

    pub fn clear(&mut self) {
        self.particles.clear();
    }
}

#[system(ParticleUpdate)]
pub fn particle_update(particle: Query<&mut ParticleEmitter>, time: Res<Time>) {
    for mut particle in particle.iter() {
        particle.update(time.delta_seconds);
    }
}
