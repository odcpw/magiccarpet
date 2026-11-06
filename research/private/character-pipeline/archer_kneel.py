"""Grounded one-knee posture for the selected Meshy-grip archer.

`apply_kneel(rig, mesh)` operates on the current pose. It moves the root and leg
bones only; the caller retains ownership of the upper-body aiming pose.
"""
import math
from mathutils import Matrix,Vector
import bpy


def _point(rig,name):
 return rig.matrix_world@rig.pose.bones[name].head


def _rotate_segment(rig,name,current,desired):
 if current.length<1e-7 or desired.length<1e-7:raise ValueError('Degenerate leg segment')
 bone=rig.pose.bones[name];rotation=current.rotation_difference(desired)
 basis=rig.matrix_world.to_quaternion();rotation=basis.inverted()@rotation@basis
 head=bone.matrix.translation.copy()
 bone.matrix=Matrix.Translation(head)@rotation.to_matrix().to_4x4()@Matrix.Translation(-head)@bone.matrix
 bpy.context.view_layer.update()


def _solve_leg(rig,upper,lower,foot,target,pole):
 a,b,c=(_point(rig,n) for n in (upper,lower,foot));l1=(b-a).length;l2=(c-b).length
 direction=target-a;dist=direction.length
 if dist<1e-7:raise ValueError('Foot target coincides with hip')
 if not abs(l1-l2)+1e-5<dist<l1+l2-1e-5:raise ValueError('Kneel foot target unreachable')
 unit=direction/dist;along=(l1*l1-l2*l2+dist*dist)/(2*dist);height=math.sqrt(max(0,l1*l1-along*along));bend=pole-unit*pole.dot(unit)
 if bend.length<1e-7:raise ValueError('Degenerate knee pole')
 knee=a+along*unit+height*bend.normalized()
 _rotate_segment(rig,upper,b-a,knee-a)
 b,c=(_point(rig,n) for n in (lower,foot));_rotate_segment(rig,lower,c-b,target-b)
 return l1,l2


def apply_kneel(rig,mesh):
 """Lower the current pose, plant the left foot and set the right knee back.

 Returns world-space joint positions and requested segment lengths in model
 units; the caller must inspect the evaluated character and floor contact.
 """
 if rig.pose.bones['Hips'].parent:raise ValueError('Expected root Hips bone')
 root=rig.pose.bones['Hips'];root_head=_point(rig,'Hips');target_root_z=.58
 basis=rig.matrix_world.to_3x3()@root.bone.matrix_local.to_3x3()
 root.location+=basis.inverted()@Vector((0,0,target_root_z-root_head.z));bpy.context.view_layer.update()
 forward=(rig.matrix_world.to_3x3()@Vector((0,-1,0))).normalized();back=-forward
 left_hip=_point(rig,'LeftUpLeg');left_target=left_hip+forward*.27;left_target.z=.146
 front_lengths=_solve_leg(rig,'LeftUpLeg','LeftLeg','LeftFoot',left_target,forward+Vector((0,0,.20)))
 # Keep the planted shoe parallel to its undistorted bind orientation.
 lfoot=_point(rig,'LeftFoot');ltoe=_point(rig,'LeftToeBase')
 rest=(rig.matrix_world.to_3x3()@(rig.data.bones['LeftToeBase'].head_local-rig.data.bones['LeftFoot'].head_local))
 _rotate_segment(rig,'LeftFoot',ltoe-lfoot,rest)
 left_toe=rig.pose.bones['LeftToeBase'];left_foot=rig.pose.bones['LeftFoot']
 left_toe.matrix=left_foot.matrix@left_foot.bone.matrix_local.inverted()@left_toe.bone.matrix_local
 bpy.context.view_layer.update()
 # Rear thigh lowers the knee; the shin reaches back at near-floor height.
 hip=_point(rig,'RightUpLeg');knee=_point(rig,'RightLeg');ankle=_point(rig,'RightFoot')
 thigh=(knee-hip).length;shin=(ankle-knee).length
 direction=(back*.015+Vector((0,0,-.38))).normalized()
 _rotate_segment(rig,'RightUpLeg',knee-hip,direction*thigh)
 knee=_point(rig,'RightLeg');ankle=_point(rig,'RightFoot')
 _rotate_segment(rig,'RightLeg',ankle-knee,(back*.42+Vector((0,0,.025))).normalized()*shin)
 ankle=_point(rig,'RightFoot');toe=_point(rig,'RightToeBase')
 _rotate_segment(rig,'RightFoot',toe-ankle,(back*.15+Vector((0,0,-.02))))
 right_toe=rig.pose.bones['RightToeBase'];right_foot=rig.pose.bones['RightFoot']
 right_toe.matrix=right_foot.matrix@right_foot.bone.matrix_local.inverted()@right_toe.bone.matrix_local
 bpy.context.view_layer.update()
 return {'root_z':_point(rig,'Hips').z,'front_ankle':tuple(_point(rig,'LeftFoot')),
         'rear_knee':tuple(_point(rig,'RightLeg')),'rear_ankle':tuple(_point(rig,'RightFoot')),
         'front_lengths':front_lengths,'rear_lengths':(thigh,shin)}
