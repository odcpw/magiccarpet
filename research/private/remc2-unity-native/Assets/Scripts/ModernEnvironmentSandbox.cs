using System;
using System.Linq;
using UnityEngine;

namespace MagicCarpet.Native
{
    // An environment workbench over a frozen source terrain snapshot. Its camera,
    // weather and experiments never submit gameplay input or mutate a save.
    internal sealed class ModernEnvironmentSandbox : IDisposable
    {
        internal static readonly bool Requested = Environment.GetCommandLineArgs().Contains("-mc-environment-sandbox");
        private readonly Camera camera;
        private readonly ModernTerrainMesh terrain;
        private readonly ModernWaterSystem water;
        private readonly ModernNightLighting lighting;
        private readonly Light key;
        private readonly Material sky;
        private readonly CursorLockMode previousLock;
        private readonly bool previousCursor;
        private readonly bool tour = Environment.GetCommandLineArgs().Contains("-mc-sandbox-tour");
        private readonly int study = StudyOption();
        private bool initialized, panel = true, torches = true;
        private float yaw, pitch, speed = 18, cover = .46f, started, lastImpact = -10;
        private float frameMilliseconds;
        private int lightMode, windMode, tourStage = -1;
        private bool secondReviewImpact;
        private Vector3[] lamps;
        private Vector3 lake, grove;
        private string notice = "Explore the island. Aim at water to make a splash or a steam plume.";
        private GUIStyle title, body, small, button;
        private static readonly string[] LightNames = { "Daylight", "Dusk", "Moonlight" };
        private static readonly string[] WindNames = { "Calm", "Breeze", "Gusts" };

        internal ModernEnvironmentSandbox(Camera camera, ModernTerrainMesh terrain, ModernWaterSystem water,
            ModernNightLighting lighting, Light key, Material sky)
        {
            this.camera = camera; this.terrain = terrain; this.water = water;
            this.lighting = lighting; this.key = key; this.sky = sky;
            previousLock = Cursor.lockState; previousCursor = Cursor.visible;
            Cursor.lockState = CursorLockMode.None; Cursor.visible = true;
        }

        internal void Update()
        {
            if (!initialized)
            {
                initialized = true; started = Time.unscaledTime;
                lake = FindLake(); grove = FindGrove();
                lamps = new Vector3[3];
                lamps[0]=Lamp(grove,0);
                lamps[1]=Lamp(grove+new Vector3(4,0,3),1);
                lamps[2]=Lamp(grove+new Vector3(-3,0,4),2);
                lighting.SetReviewTorches(lamps);
                SetLighting(0); SetWind(0); View(0);
                if (ModernCapture.ReviewPosition.HasValue)
                    SetPose(ModernCapture.ReviewPosition.Value,ModernCapture.ReviewRotation);
                Debug.Log($"MC_SANDBOX_READY lake={lake} grove={grove} frozen_campaign=True");
            }
            var dt = Mathf.Min(Time.unscaledDeltaTime,.1f);
            frameMilliseconds = Mathf.Lerp(frameMilliseconds,Time.unscaledDeltaTime*1000,1-Mathf.Exp(-dt*2));
            if (Input.GetKeyDown(KeyCode.F1)) panel = !panel;
            if (Input.GetKeyDown(KeyCode.F10)) Application.Quit();
            if (Input.GetKeyDown(KeyCode.Escape) || !Application.isFocused)
            { Cursor.lockState=CursorLockMode.None; Cursor.visible=true; }
            if (tour || study>=0) { Tour(Time.unscaledTime-started,dt); return; }
            if (Input.GetKeyDown(KeyCode.Alpha1)) View(0);
            if (Input.GetKeyDown(KeyCode.Alpha2)) View(1);
            if (Input.GetKeyDown(KeyCode.Alpha3)) View(2);
            if (Input.GetKeyDown(KeyCode.Alpha4)) View(3);
            if (Input.GetKeyDown(KeyCode.N)) SetLighting((lightMode+1)%3);
            if (Input.GetKeyDown(KeyCode.G)) SetWind((windMode+1)%3);
            if (Input.GetKeyDown(KeyCode.T)) ToggleTorches();
            if (Input.GetKeyDown(KeyCode.Backspace)) { water.ClearSandboxEffects(); notice="Water and steam cleared."; }
            if (Input.GetKeyDown(KeyCode.Space)) Impact(true);
            if (Input.GetKeyDown(KeyCode.R)) Impact(false);
            if (Input.GetMouseButtonDown(1)) { Cursor.lockState=CursorLockMode.Locked; Cursor.visible=false; }
            if (Input.GetMouseButtonUp(1)) { Cursor.lockState=CursorLockMode.None; Cursor.visible=true; }
            if (Input.GetMouseButton(1) && Application.isFocused)
            {
                yaw += Input.GetAxisRaw("Mouse X")*2;
                pitch = Mathf.Clamp(pitch-Input.GetAxisRaw("Mouse Y")*2,-85,85);
                camera.transform.rotation=Quaternion.Euler(pitch,yaw,0);
            }
            var move = new Vector3(Axis(KeyCode.D,KeyCode.A),Axis(KeyCode.E,KeyCode.Q),Axis(KeyCode.W,KeyCode.S));
            var previous=camera.transform.position;
            var velocity=camera.transform.right*move.x+Vector3.up*move.y+camera.transform.forward*move.z;
            var boost=Input.GetKey(KeyCode.LeftShift)?3f:Input.GetKey(KeyCode.LeftControl)?.2f:1f;
            var next=previous+Vector3.ClampMagnitude(velocity,1)*speed*boost*dt;
            next.y=Mathf.Clamp(Mathf.Max(next.y,terrain.SampleHeight(next.x,next.z)+.65f),water.CameraClearance(next.x,next.z),650);
            camera.transform.position=next;
            water.SandboxSweep(previous,next,dt);
        }

        private static float Axis(KeyCode positive, KeyCode negative) => (Input.GetKey(positive)?1:0)-(Input.GetKey(negative)?1:0);
        private Vector3 Lamp(Vector3 p,int placed)
        {
            var best=p;var score=float.PositiveInfinity;
            for(var z=-10;z<=10;z++) for(var x=-10;x<=10;x++)
            {
                var q=p+new Vector3(x,0,z);var h=terrain.SampleHeight(q.x,q.z);
                if(h<1f || terrain.IsWater(q.x,q.z) || terrain.IsWater(q.x-1,q.z)
                    ||terrain.IsWater(q.x+1,q.z)||terrain.IsWater(q.x,q.z-1)||terrain.IsWater(q.x,q.z+1)) continue;
                var occupied=false;
                for(var i=0;i<placed;i++)
                    occupied|=new Vector2(q.x-lamps[i].x,q.z-lamps[i].z).sqrMagnitude<2.25f;
                if(occupied) continue;
                var value=x*x+z*z+Mathf.Abs(h-1)*.1f;
                if(value<score) { score=value;best=q; }
            }
            if(float.IsPositiveInfinity(score)) throw new InvalidOperationException("No dry ground for sandbox torch");
            return new Vector3(best.x,terrain.SampleHeight(best.x,best.z)+1.6f,best.z);
        }

        private Vector3 FindLake()
        {
            var best=new Vector3(125,0,842); var score=float.NegativeInfinity;
            for(var z=780;z<910;z+=4) for(var x=60;x<210;x+=4)
            {
                if(!terrain.IsWater(x,z)) continue;
                var clear=true;
                for(var dz=-8;dz<=8;dz+=8) for(var dx=-8;dx<=8;dx+=8)
                    clear &= terrain.IsWater(x+dx,z+dz);
                if(!clear) continue;
                var candidate=-Vector2.Distance(new Vector2(x,z),new Vector2(125,842));
                if(candidate>score) { score=candidate; best=new Vector3(x,0,z); }
            }
            return best;
        }

        private Vector3 FindGrove()
        {
            var best=new Vector3(180,0,842); var score=float.PositiveInfinity;
            for(var z=780;z<910;z+=2) for(var x=70;x<230;x+=2)
            {
                if(terrain.IsWater(x,z)) continue;
                var h=terrain.SampleHeight(x,z);
                var slope=Mathf.Abs(terrain.SampleHeight(x+3,z)-h)+Mathf.Abs(terrain.SampleHeight(x,z+3)-h);
                var candidate=Vector2.Distance(new Vector2(x,z),new Vector2(lake.x,lake.z))+slope*15+Mathf.Abs(h-4)*2;
                if(candidate<score) { score=candidate; best=new Vector3(x,h,z); }
            }
            return best;
        }

        private void SetPose(Vector3 position, Quaternion rotation)
        {
            position.y=Mathf.Max(water.CameraClearance(position.x,position.z),Mathf.Max(position.y,terrain.SampleHeight(position.x,position.z)+.65f));
            camera.transform.SetPositionAndRotation(position,rotation);
            pitch=Mathf.DeltaAngle(0,rotation.eulerAngles.x); yaw=rotation.eulerAngles.y;
            camera.fieldOfView=67;
        }

        private void Look(Vector3 position, Vector3 target)
        {
            position.y=Mathf.Max(water.CameraClearance(position.x,position.z),Mathf.Max(position.y,terrain.SampleHeight(position.x,position.z)+.65f));
            SetPose(position,Quaternion.LookRotation(target-position));
        }

        internal void View(int index)
        {
            switch(index)
            {
                case 0: Look(lake+new Vector3(-18,5,-12),lake+Vector3.up*.4f); break;
                case 1: Look(grove+new Vector3(-7,3,-6),grove+Vector3.up*1.4f); break;
                case 2: Look(lake+new Vector3(-90,100,-80),grove); break;
                default: Look(lake+new Vector3(0,5,0),lake+new Vector3(-38,53,79)); break;
            }
            notice=new[] { "Shoreline", "Grove and lanterns", "Island overlook", "Clouds and sky" }[Mathf.Clamp(index,0,3)];
            Debug.Log($"MC_SANDBOX_VIEW index={index} position={camera.transform.position}");
        }

        internal void SetLighting(int mode)
        {
            lightMode=Mathf.Clamp(mode,0,2);
            var day=lightMode==0; var dusk=lightMode==1;
            var direction=(day?new Vector3(-.5f,.65f,.55f):dusk?new Vector3(-.65f,.12f,.6f):new Vector3(-.38f,.48f,.79f)).normalized;
            key.transform.rotation=Quaternion.LookRotation(-direction);
            key.color=day?new Color(1,.94f,.82f):dusk?new Color(1,.54f,.27f):new Color(.76f,.86f,1);
            key.intensity=day?1.5f:dusk?1.0f:.48f;
            sky.SetVector("_MoonDirection",direction);
            sky.SetFloat("_Daylight",day?1:dusk?.75f:0);
            sky.SetColor("_MoonColor",day?new Color(1,.96f,.85f):dusk?new Color(1,.56f,.25f):new Color(.70f,.78f,.9f));
            sky.SetColor("_ZenithColor",day?new Color(.10f,.27f,.49f):dusk?new Color(.055f,.10f,.23f):new Color(.012f,.027f,.065f));
            sky.SetColor("_HorizonColor",day?new Color(.57f,.69f,.77f):dusk?new Color(.58f,.29f,.17f):new Color(.09f,.13f,.19f));
            sky.SetFloat("_CloudAmount",cover);
            RenderSettings.ambientSkyColor=day?new Color(.48f,.62f,.78f):dusk?new Color(.24f,.29f,.42f):new Color(.11f,.16f,.26f);
            RenderSettings.ambientEquatorColor=day?new Color(.32f,.39f,.43f):dusk?new Color(.25f,.21f,.20f):new Color(.075f,.10f,.14f);
            RenderSettings.ambientGroundColor=day?new Color(.16f,.19f,.13f):dusk?new Color(.12f,.10f,.085f):new Color(.035f,.045f,.045f);
            // The periodic island has a finite presentation radius. Match its
            // atmospheric extinction to the sky horizon before that boundary.
            RenderSettings.fogColor=sky.GetColor("_HorizonColor");
            RenderSettings.fogDensity=day?.0038f:dusk?.0040f:.0038f;
            Debug.Log($"MC_SANDBOX_LIGHT mode={LightNames[lightMode]}");
        }

        private void SetWind(int mode) { windMode=mode; water.SetSandboxWind(mode); }
        private void ToggleTorches() { torches=!torches; lighting.SetReviewTorches(torches?lamps:Array.Empty<Vector3>()); }

        // Testable targeting uses the rendered heightfield, not nonexistent Unity
        // colliders. Land between camera and water must block the experiment.
        internal static bool TryWaterTarget(Vector3 origin, Vector3 direction, ModernTerrainMesh terrain, out Vector3 hit)
        {
            hit=default;
            if(!float.IsFinite(origin.x)||!float.IsFinite(origin.y)||!float.IsFinite(origin.z)
                ||!float.IsFinite(direction.x)||!float.IsFinite(direction.y)||!float.IsFinite(direction.z)||direction.y>=-.001f) return false;
            direction.Normalize();
            var distance=(.15f-origin.y)/direction.y;
            if(distance<=0||distance>250) return false;
            hit=origin+direction*distance;
            if(!terrain.IsWater(hit.x,hit.z)) return false;
            var steps=Mathf.CeilToInt(distance/1.5f);
            for(var i=1;i<steps;i++)
            {
                var p=Vector3.Lerp(origin,hit,i/(float)steps);
                if(!terrain.IsWater(p.x,p.z)&&terrain.SampleHeight(p.x,p.z)>p.y) return false;
            }
            return true;
        }

        private void Impact(bool hot)
        {
            if(Time.unscaledTime-lastImpact<.35f) return;
            if(!TryWaterTarget(camera.transform.position,camera.transform.forward,terrain,out var hit))
            { notice="Look down at open water within 250 m."; return; }
            lastImpact=Time.unscaledTime;
            water.SandboxImpact(hit,hot);
            notice=hot?"2 MJ quench — fly through the cloud to disturb it.":"Ripple and splash.";
        }

        private void Tour(float time, float dt)
        {
            var elapsed=Mathf.Max(0,time-4);
            var stage=study>=0?study:Mathf.Min(10,(int)(elapsed/10));
            var local=study>=0?elapsed:elapsed-stage*10;
            if(stage!=tourStage)
            {
                tourStage=stage; SetLighting(stage<2?stage:2); SetWind(stage==1?1:0);
                if(stage==3) water.SandboxImpact(lake,true);
                if(stage==4 && study>=0) { water.ClearSandboxEffects();secondReviewImpact=false; }
                if(stage==6 || stage==7) { SetLighting(0); SetWind(stage==6?2:1); water.ClearSandboxEffects(); }
                if(stage==8) { water.ClearSandboxEffects();water.SandboxImpact(lake,true);secondReviewImpact=false; }
                if(stage==9 || stage==12)
                {
                    water.ClearSandboxEffects();secondReviewImpact=false;
                    if(study<0) { water.SandboxImpact(ReviewWaterNear(grove),true);secondReviewImpact=true; }
                }
                if(stage==10) { SetLighting(1);SetWind(2);water.ClearSandboxEffects(); }
                if(stage==11)
                {
                    water.ClearSandboxEffects();
                    foreach(var offset in new[] { new Vector3(-12,0,-8),new Vector3(8,0,-8),new Vector3(-12,0,12),new Vector3(8,0,12) })
                        water.SandboxImpact(ReviewWaterNear(lake+offset),true);
                }
            }
            var previous=camera.transform.position;
            if(stage==0) Look(lake+new Vector3(-24+local*.7f,5,-15),grove+Vector3.up*3);
            else if(stage==1) Look(grove+new Vector3(-10+local*.5f,3,-8),grove+Vector3.up*1.4f);
            else if(stage==2) Look(grove+new Vector3(Mathf.Cos(local*.1f)*5,2.3f,Mathf.Sin(local*.1f)*5),lamps[0]);
            else if(stage==3) Look(lake+new Vector3(-8,3,-5),water.CloudCenter??lake+Vector3.up);
            else if(stage==4)
            {
                if(study>=0 && !secondReviewImpact)
                {
                    // Let the initial weather settle before this isolated study;
                    // otherwise its plume is blown into the hill before arrival.
                    if(local<12) { Look(lake+new Vector3(-8,6,-5),lake+Vector3.up*2);return; }
                    water.SandboxImpact(lake,true);secondReviewImpact=true;
                }
                var center=water.CloudCenter??lake+Vector3.up*2;
                var direction=new Vector3(.8f,0,.6f);
                // Reuse the preceding quench in the tour. Standalone studies
                // allow the cloud to rise above the wave-safe flight floor first.
                var flightTime=study>=0?Mathf.Max(0,local-20):local;
                var p=center+direction*Mathf.Lerp(-7,8,flightTime/10);
                Look(p,p+direction); water.SandboxSweep(previous,camera.transform.position,dt);
            }
            else if(stage==5) Look(lake+Vector3.up*5,lake+new Vector3(-38+local,53,79));
            else if(stage==6) Look(grove+new Vector3(-7,2,-6),grove+new Vector3(-1,0,0));
            else if(stage==7) Look(lake+new Vector3(-90+local,100,-80),grove);
            else if(stage==8)
            {
                var other=ReviewWaterNear(lake+new Vector3(-22,0,0));
                if(local>=3 && !secondReviewImpact) { water.SandboxImpact(other,true); secondReviewImpact=true; }
                Look(lake+new Vector3(-12,9,-27),lake+new Vector3(-10,2,0));
            }
            else if(stage==9 || stage==12)
            {
                var wet=ReviewWaterNear(grove);
                if(!secondReviewImpact && local>=12) { water.SandboxImpact(wet,true);secondReviewImpact=true; }
                if(!secondReviewImpact) { Look(grove+new Vector3(-7,3,-6),lamps[0]);return; }
                var center=water.CloudCenter??wet+Vector3.up;
                var lamp=lamps[0]+Vector3.up*.3f;
                if(stage==9) Look(center+(center-lamp)*2,lamp);
                else Look(lamp+(lamp-center)*2,center);
            }
            else if(stage==10) Look(grove+new Vector3(-7,2,-6),grove+new Vector3(-1,0,0));
            else
            {
                var center=(water.CloudCenter??lake)-new Vector3(8,0,12);
                Look(center+new Vector3(-12,9,-27),center+Vector3.up*2);
            }
        }

        private static int StudyOption()
        {
            var args=Environment.GetCommandLineArgs();var i=Array.IndexOf(args,"-mc-sandbox-study");
            if(i<0) return -1;
            if(i+1>=args.Length || !int.TryParse(args[i+1],out var stage) || stage<0 || stage>12)
                throw new ArgumentException("Sandbox study must be a view from 0 to 12");
            return stage;
        }

        private Vector3 ReviewWaterNear(Vector3 target)
        {
            var best=lake;var distance=float.PositiveInfinity;
            for(var z=-28;z<=28;z+=2) for(var x=-28;x<=28;x+=2)
            {
                var p=target+new Vector3(x,0,z);p.y=.15f;
                if(!terrain.IsWater(p.x,p.z)||!terrain.IsWater(p.x-1,p.z)||!terrain.IsWater(p.x+1,p.z)
                    ||!terrain.IsWater(p.x,p.z-1)||!terrain.IsWater(p.x,p.z+1)) continue;
                var d=x*x+z*z;
                if(d<distance) { distance=d;best=p; }
            }
            return best;
        }

        internal void Draw()
        {
            if(!initialized || ModernCapture.HideHud) return;
            title??=new GUIStyle(GUI.skin.label) { fontSize=22,fontStyle=FontStyle.Bold,normal={textColor=new Color(.87f,.94f,.93f)} };
            body??=new GUIStyle(GUI.skin.label) { fontSize=14,wordWrap=true,normal={textColor=new Color(.76f,.84f,.84f)} };
            small??=new GUIStyle(body) { fontSize=12,normal={textColor=new Color(.53f,.66f,.67f)} };
            button??=new GUIStyle(GUI.skin.button) { fontSize=13,fixedHeight=30 };
            if(panel)
            {
                GUI.color=new Color(.025f,.045f,.05f,.94f); GUI.DrawTexture(new Rect(20,20,326,530),Texture2D.whiteTexture); GUI.color=Color.white;
                GUILayout.BeginArea(new Rect(38,34,290,500));
                GUILayout.Label("MAGIC CARPET",small);
                GUILayout.Label("Environment sandbox",title);
                GUILayout.Space(10); GUILayout.Label("LIGHT",small);
                Row(LightNames,lightMode,SetLighting);
                GUILayout.Space(6); GUILayout.Label("WIND",small);
                Row(WindNames,windMode,SetWind);
                GUILayout.Label($"{water.WindVelocity.magnitude:F1} m/s  ·  Cloud cover {cover:P0}",body);
                var next=GUILayout.HorizontalSlider(cover,0,1);
                if(!Mathf.Approximately(next,cover)) { cover=next; sky.SetFloat("_CloudAmount",cover); }
                GUILayout.Space(8); GUILayout.Label("EXPLORE",small);
                Row(new[] { "Shore  1", "Grove  2", "Overlook  3" },-1,View);
                GUILayout.Label("RMB look · WASD fly · Q/E descend/rise\nShift fast · Ctrl precise · 4 sky",body);
                GUILayout.Space(8);
                GUILayout.Label("Space  Steam quench     R  Splash\nT  Torches     N  Light     G  Wind\nBackspace  Clear water and steam",body);
                GUILayout.Space(8); GUILayout.Label(notice,body);
                GUILayout.FlexibleSpace(); GUILayout.Label("F1 hide panel · Esc release mouse · F10 exit",small);
                GUILayout.EndArea();
            }
            else GUI.Label(new Rect(24,20,420,28),"ENVIRONMENT SANDBOX  /  F1 controls",body);
            GUI.Label(new Rect(24,Screen.height-32,1100,24),$"{LightNames[lightMode]}  ·  {WindNames[windMode]}  ·  {frameMilliseconds:F1} ms  ·  Environment only; campaign paused",small);
            GUI.color=new Color(.85f,.95f,.95f,.65f);
            GUI.DrawTexture(new Rect(Screen.width/2f-4,Screen.height/2f,8,1),Texture2D.whiteTexture);
            GUI.DrawTexture(new Rect(Screen.width/2f,Screen.height/2f-4,1,8),Texture2D.whiteTexture); GUI.color=Color.white;
        }

        private void Row(string[] labels, int selected, Action<int> choose)
        {
            GUILayout.BeginHorizontal();
            for(var i=0;i<labels.Length;i++)
            {
                GUI.backgroundColor=i==selected?new Color(.28f,.6f,.56f):new Color(.25f,.30f,.32f);
                if(GUILayout.Button(labels[i],button)) choose(i);
            }
            GUI.backgroundColor=Color.white; GUILayout.EndHorizontal();
        }

        public void Dispose() { Cursor.lockState=previousLock; Cursor.visible=previousCursor; }
    }
}
